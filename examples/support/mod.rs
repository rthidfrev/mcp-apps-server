//! Application types shared by the adapter and custom-handler examples.
use mcp_apps_server::{MetaObject, MetadataError, ResourceUi, UiCsp};
use rmcp::{ErrorData, handler::server::wrapper::Json, schemars::JsonSchema};
use serde::{Deserialize, Serialize};

// The handwritten resource uses these; the derive embeds equivalent literals.
#[allow(dead_code)]
pub const DASHBOARD_URI: &str = "ui://inventory/dashboard.html";
#[allow(dead_code)]
pub const DASHBOARD_HTML: &str = include_str!("../dashboard.html");

/// Identity of the item whose demonstration snapshot is requested.
#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct SnapshotInput {
    /// Nonempty stock keeping unit (SKU), an application-owned item identifier.
    pub sku: String,
}
/// A typed result also available as ordinary JSON text without UI support.
#[derive(Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct Snapshot {
    pub sku: String,
    pub available: u32,
    pub reserved: u32,
}
/// Shared demonstration logic used by both registration styles.
pub fn read_snapshot(input: SnapshotInput) -> Result<Json<Snapshot>, ErrorData> {
    if input.sku.trim().is_empty() {
        return Err(ErrorData::invalid_params("SKU must not be blank", None));
    }
    Ok(Json(Snapshot {
        sku: input.sku,
        available: 12,
        reserved: 3,
    }))
}
/// Request no external connections/assets and a visible host border.
pub fn dashboard_ui() -> ResourceUi {
    let mut csp = UiCsp::default();
    csp.connect_domains = Some(vec![]);
    csp.resource_domains = Some(vec![]);
    let mut ui = ResourceUi::default();
    ui.csp = Some(csp);
    ui.prefers_border = Some(true);
    ui
}

/// Illustrative extension annotations; a consuming SDK would own this wire type.
#[derive(Serialize)]
struct DisplayPreferences {
    compact: bool,
}

/// Prepare fixed extension data without introducing host types into the router.
pub fn dashboard_meta() -> Result<MetaObject, MetadataError> {
    let mut meta = MetaObject::default();
    meta.0.insert(
        "example/display".into(),
        serde_json::to_value(DisplayPreferences { compact: true })?,
    );
    Ok(meta)
}
