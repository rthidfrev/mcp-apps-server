//! Typed inventory server using StaticUiResource and AppServer.
//! Run `cargo run --example server --features server,macros`.
//! HTML remains static, without a browser bridge or live refresh.
#![forbid(unsafe_code)]
mod support;
use mcp_apps_server::{AppServer, StaticUiResource, app_router};
use rmcp::{
    ErrorData, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    model::Implementation,
    tool,
};
use support::{Snapshot, SnapshotInput, dashboard_meta, dashboard_ui, read_snapshot};

#[derive(StaticUiResource)]
#[ui_resource(id = "inventory/dashboard.html", name = "inventory-dashboard", html_file = "dashboard.html", ui = dashboard_ui, meta = dashboard_meta)]
struct Dashboard;

pub struct Inventory;
#[app_router]
impl Inventory {
    #[tool(
        name = "inventory_snapshot",
        description = "Read a static inventory snapshot",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    #[ui(resource = Dashboard)]
    async fn snapshot(
        &self,
        Parameters(input): Parameters<SnapshotInput>,
    ) -> Result<Json<Snapshot>, ErrorData> {
        read_snapshot(input)
    }
}

/// Prepare declarations without starting a transport or runtime.
pub fn inventory_server() -> Result<AppServer<Inventory>, Box<dyn std::error::Error>> {
    Ok(
        AppServer::new(Inventory, Inventory::app_router()?)?.with_server_info(Implementation::new(
            "inventory-example",
            env!("CARGO_PKG_VERSION"),
        )),
    )
}
// Integration tests import this module; only the standalone binary runs stdio.
#[allow(dead_code)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    inventory_server()?
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
