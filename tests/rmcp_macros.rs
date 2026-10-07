//! Check that complete typed metadata is accepted by rmcp's existing tool macros.

#![forbid(unsafe_code)]

use mcp_apps_server::{ToolUi, UiResourceUri};
use rmcp::{ServerHandler, model::MetaObject, tool, tool_router};
use serde_json::json;

struct MacroTools;

impl ServerHandler for MacroTools {}

fn dashboard_meta() -> MetaObject {
    // Only a fixed fixture is evaluated by the generated infallible tool factory.
    ToolUi::new(UiResourceUri::new("ui://example/dashboard").expect("valid fixed fixture"))
        .to_meta()
        .expect("serializable fixed declaration")
}

#[tool_router]
impl MacroTools {
    #[tool(meta = dashboard_meta())]
    async fn dashboard(&self) -> String {
        "Dashboard text fallback".into()
    }
}

#[test]
fn existing_tool_macro_accepts_prepared_ui_metadata() {
    let tools = MacroTools::tool_router().list_all();
    assert_eq!(tools.len(), 1);
    assert_eq!(
        serde_json::to_value(tools[0].meta.as_ref().unwrap()).unwrap(),
        json!({
            "ui": {"resourceUri": "ui://example/dashboard"}
        })
    );
}
