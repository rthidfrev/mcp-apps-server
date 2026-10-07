//! Custom ServerHandler using AppRouter and a handwritten resource contract.
//! Run `cargo run --example custom_server --features server`.
#![forbid(unsafe_code)]
mod support;
use mcp_apps_server::{
    AppRouter, MetadataError, ResourceUi, StaticUiResource, UiResource, UiResourceUri, UiUriError,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    model::*,
    service::RequestContext,
    tool, tool_router,
};
use support::{
    DASHBOARD_HTML, DASHBOARD_URI, Snapshot, SnapshotInput, dashboard_meta, dashboard_ui,
    read_snapshot,
};
struct Inventory;
#[tool_router]
impl Inventory {
    #[tool(
        name = "inventory_snapshot",
        description = "Read a static inventory snapshot",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn snapshot(
        &self,
        Parameters(input): Parameters<SnapshotInput>,
    ) -> Result<Json<Snapshot>, ErrorData> {
        read_snapshot(input)
    }
}
struct Dashboard;
impl StaticUiResource for Dashboard {
    fn resource() -> Result<UiResource, UiUriError> {
        Ok(UiResource::new(
            UiResourceUri::new(DASHBOARD_URI)?,
            "inventory-dashboard",
        ))
    }
    fn html() -> &'static str {
        DASHBOARD_HTML
    }
    fn ui() -> ResourceUi {
        dashboard_ui()
    }
    fn meta() -> Result<MetaObject, MetadataError> {
        dashboard_meta()
    }
}
pub struct InventoryServer {
    state: Inventory,
    router: AppRouter<Inventory>,
}
impl InventoryServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut router = AppRouter::from_tool_router(Inventory::tool_router());
        router.register_static::<Dashboard>()?;
        router.associate_static_tool::<Dashboard>("inventory_snapshot")?;
        router.validate()?;
        Ok(Self {
            state: Inventory,
            router,
        })
    }
}
impl ServerHandler for InventoryServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::default();
        self.router.declare_capabilities(&mut info.capabilities);
        info.server_info = Implementation::new("inventory-example", env!("CARGO_PKG_VERSION"));
        info
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.router.get_tool(name)
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.router.list_tools(request)
    }
    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.router.list_resources(request)
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Public read-only demo. A real application can authorize here before dispatch.
        self.router.call_tool(&self.state, request, context).await
    }
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        if !self.router.contains_resource(&request.uri) {
            // An existing server can route its other resource namespaces here.
            return Err(ErrorData::resource_not_found(
                "Unknown inventory resource",
                None,
            ));
        }
        self.router
            .read_resource(&self.state, request, context)
            .await
    }
}
// Integration tests import this module; only the standalone binary runs stdio.
#[allow(dead_code)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    InventoryServer::new()?
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
