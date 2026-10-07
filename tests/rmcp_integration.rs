//! Actual rmcp byte-transport checks of consumer examples, not host rendering.
#![cfg(feature = "server")]
#![forbid(unsafe_code)]
#![allow(
    clippy::duplicate_mod,
    reason = "Compile both complete example entry points; each imports the same support module"
)]
#[cfg(feature = "macros")]
#[path = "../examples/server.rs"]
mod adapter;
#[path = "../examples/custom_server.rs"]
mod custom;
use mcp_apps_server::{UI_EXTENSION_ID, UI_MIME_TYPE};
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolRequestParams, ReadResourceRequestParams},
};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::timeout;
const DASHBOARD_URI: &str = "ui://inventory/dashboard.html";

async fn observe(server: impl ServerHandler) -> Value {
    let (left, right) = tokio::io::duplex(16 * 1024);
    let (server, client) = timeout(Duration::from_secs(5), async {
        tokio::join!(server.serve(left), ().serve(right))
    })
    .await
    .expect("initialization finishes");
    let server = server.expect("server initializes");
    let client = client.expect("client initializes");
    let capabilities = serde_json::to_value(&client.peer_info().unwrap().capabilities).unwrap();
    let observed = timeout(Duration::from_secs(5), async {
        let tools = client.list_all_tools().await?;
        let resources = client.list_all_resources().await?;
        let contents = client.read_resource(ReadResourceRequestParams::new(DASHBOARD_URI)).await?;
        let args = json!({"sku": "bolt-M6"}).as_object().unwrap().clone();
        let result = client.call_tool(CallToolRequestParams::new("inventory_snapshot").with_arguments(args)).await?;
        let bad_args = json!({"sku": "bolt-M6", "unexpected": true}).as_object().unwrap().clone();
        // rmcp's ToolRouter converts INVALID_PARAMS extraction failures into a
        // tool result with isError=true, rather than a JSON-RPC request error.
        let rejected = client.call_tool(CallToolRequestParams::new("inventory_snapshot").with_arguments(bad_args)).await?;
        Ok::<_, rmcp::ServiceError>(json!({"capabilities": capabilities, "tools": tools, "resources": resources, "contents": contents, "result": result, "rejected": rejected}))
    }).await;
    let shutdown = timeout(Duration::from_secs(5), async {
        tokio::join!(client.cancel(), server.cancel())
    })
    .await
    .expect("peers stop");
    shutdown.0.expect("client stops");
    shutdown.1.expect("server stops");
    observed
        .expect("requests finish")
        .expect("requests succeed")
}
fn check_snapshot(observed: &Value) {
    assert_eq!(
        observed["capabilities"]["extensions"][UI_EXTENSION_ID],
        json!({})
    );
    assert_eq!(
        observed["tools"][0]["_meta"]["ui"],
        json!({"resourceUri": DASHBOARD_URI})
    );
    let schema = &observed["tools"][0]["inputSchema"];
    assert_eq!(schema["properties"]["sku"]["type"], "string");
    assert_eq!(schema["required"], json!(["sku"]));
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        observed["tools"][0]["outputSchema"]["properties"]["sku"]["type"],
        "string"
    );
    assert_eq!(observed["resources"][0]["uri"], DASHBOARD_URI);
    assert_eq!(observed["resources"][0]["mimeType"], UI_MIME_TYPE);
    let content = &observed["contents"]["contents"][0];
    assert_eq!(content["uri"], DASHBOARD_URI);
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    assert_eq!(content["text"], include_str!("../examples/dashboard.html"));
    assert_eq!(
        content["_meta"],
        json!({
            "example/display": {"compact": true},
            "ui": {"csp": {"connectDomains": [], "resourceDomains": []}, "prefersBorder": true}
        })
    );
    let snapshot = json!({"sku": "bolt-M6", "available": 12, "reserved": 3});
    assert_eq!(observed["result"]["structuredContent"], snapshot);
    assert_eq!(observed["result"]["content"][0]["type"], "text");
    assert_eq!(
        serde_json::from_str::<Value>(observed["result"]["content"][0]["text"].as_str().unwrap())
            .unwrap(),
        snapshot
    );
    let rejected = &observed["rejected"];
    assert_eq!(rejected["isError"], true);
    assert!(rejected["structuredContent"].is_null());
    assert_eq!(rejected["content"][0]["type"], "text");
    let diagnostic = rejected["content"][0]["text"].as_str().unwrap();
    assert!(diagnostic.contains("unknown field"));
    assert!(diagnostic.contains("unexpected"));
}
#[tokio::test]
async fn handwritten_consumer_serves_typed_snapshot_without_ui_client_support() {
    check_snapshot(&observe(custom::InventoryServer::new().unwrap()).await);
}
#[cfg(feature = "macros")]
#[tokio::test]
async fn adapter_and_custom_handler_publish_the_same_contract() {
    let manual = observe(custom::InventoryServer::new().unwrap()).await;
    let derived = observe(adapter::inventory_server().unwrap()).await;
    check_snapshot(&derived);
    assert_eq!(derived, manual);
}

mod dynamic {
    use super::*;
    use mcp_apps_server::{AppRouter, UiResource, UiResourceRoute, UiResourceUri};
    use rmcp::{ErrorData, RoleServer, model::*, service::RequestContext};
    use std::sync::{Arc, Mutex};

    type Observations = Arc<Mutex<Vec<(String, Value)>>>;
    struct ContextServer {
        router: AppRouter<Observations>,
        seen: Observations,
    }
    impl ServerHandler for ContextServer {
        fn get_info(&self) -> ServerConfig {
            let mut info = ServerConfig::default();
            self.router.declare_capabilities(&mut info.capabilities);
            info
        }
        async fn read_resource(
            &self,
            request: ReadResourceRequestParams,
            context: RequestContext<RoleServer>,
        ) -> Result<ReadResourceResponse, ErrorData> {
            self.seen.lock().unwrap().push((
                context.id.to_string(),
                context
                    .meta
                    .get("example/access")
                    .cloned()
                    .unwrap_or(Value::Null),
            ));
            self.router
                .read_resource(&self.seen, request, context)
                .await
        }
    }
    #[tokio::test]
    async fn dynamic_callback_receives_original_context_and_preserves_denial() {
        let seen: Observations = Arc::new(Mutex::new(vec![]));
        let resource = UiResource::new(
            UiResourceUri::new("ui://context/view.html").unwrap(),
            "context-view",
        );
        let mut router = AppRouter::new();
        router
            .register_resource(UiResourceRoute::new(
                resource,
                |seen: &Observations, context| {
                    Box::pin(async move {
                        let marker = context
                            .meta
                            .get("example/access")
                            .cloned()
                            .unwrap_or(Value::Null);
                        seen.lock()
                            .unwrap()
                            .push((context.id.to_string(), marker.clone()));
                        if marker != "permitted" {
                            return Err(ErrorData::new(
                                ErrorCode(-32042),
                                "Read denied by application",
                                Some(json!({"policy": "inventory"})),
                            ));
                        }
                        Ok("<!doctype html><html><body>Authorized snapshot</body></html>".into())
                    })
                },
            ))
            .unwrap();
        let (left, right) = tokio::io::duplex(16 * 1024);
        let server = ContextServer {
            router,
            seen: seen.clone(),
        };
        let (server, client) = timeout(Duration::from_secs(5), async {
            tokio::join!(server.serve(left), ().serve(right))
        })
        .await
        .unwrap();
        let server = server.unwrap();
        let client = client.unwrap();
        let observed = timeout(Duration::from_secs(5), async {
            let meta: RequestMetaObject =
                serde_json::from_value(json!({"example/access": "permitted"})).unwrap();
            let allowed = client
                .read_resource(
                    ReadResourceRequestParams::new("ui://context/view.html").with_meta(meta),
                )
                .await?;
            let denied = client
                .read_resource(ReadResourceRequestParams::new("ui://context/view.html"))
                .await;
            Ok::<_, rmcp::ServiceError>((allowed, denied))
        })
        .await;
        let shutdown = timeout(Duration::from_secs(5), async {
            tokio::join!(client.cancel(), server.cancel())
        })
        .await
        .unwrap();
        shutdown.0.unwrap();
        shutdown.1.unwrap();
        let (allowed, denied) = observed.unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(&allowed.contents[0]).unwrap()["mimeType"],
            UI_MIME_TYPE
        );
        let rmcp::ServiceError::McpError(error) = denied.unwrap_err() else {
            panic!("application denial must remain an MCP error")
        };
        assert_eq!(error.code, ErrorCode(-32042));
        assert_eq!(error.message, "Read denied by application");
        assert_eq!(error.data, Some(json!({"policy": "inventory"})));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 4);
        assert_eq!(
            seen[0], seen[1],
            "callback retains outer handler request ID and metadata"
        );
        assert_eq!(seen[2], seen[3]);
        assert_ne!(
            seen[0].0, seen[2].0,
            "separate reads retain distinct request IDs"
        );
    }
}
