//! Typed MCP Apps registration and resources for rmcp servers.
//!
//! Start with [`StaticUiResource`] for embedded HTML. With the `server` feature,
//! `AppRouter` groups rmcp tools and resources and checks their associations;
//! `AppServer` provides the common server handling. With `macros`, the optional
//! derive declares fixed views and `app_router` associates each UI tool with its
//! view in one place. Manual declarations and routing remain directly usable.
//! Consumers choose their runtime and transport and own business logic/access checks.
//!
//! # Simple application (`server` and `macros`)
//!
//! The consumer enables rmcp's `server` and `macros` features. The HTML below is
//! this project's bundled example; use a path relative to your own declaring file.
//! rmcp generates both schemas from the Rust models; no input schema JSON is needed.
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(all(feature = "server", feature = "macros"))] {
//! use mcp_apps_server::{AppServer, StaticUiResource, app_router};
//! use rmcp::{tool, schemars::JsonSchema, ServerHandler,
//!     handler::server::wrapper::{Json, Parameters}};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(StaticUiResource)]
//! #[ui_resource(id = "inventory/dashboard", name = "dashboard",
//!               html_file = "../examples/dashboard.html")]
//! struct Dashboard;
//!
//! #[derive(Deserialize, JsonSchema)]
//! #[schemars(crate = "rmcp::schemars")]
//! #[serde(deny_unknown_fields)]
//! struct Input { sku: String }
//! #[derive(Serialize, JsonSchema)]
//! #[schemars(crate = "rmcp::schemars")]
//! struct Snapshot { sku: String, available: u32 }
//! struct Inventory;
//! #[app_router]
//! impl Inventory {
//!     #[tool(description = "Read inventory")]
//!     #[ui(resource = Dashboard)]
//!     fn snapshot(&self, Parameters(input): Parameters<Input>) -> Json<Snapshot> {
//!         Json(Snapshot { sku: input.sku, available: 12 })
//!     }
//! }
//! let server = AppServer::new(Inventory, Inventory::app_router()?)?;
//! assert!(server.get_info().capabilities.resources.is_some());
//! assert!(server.get_tool("snapshot").is_some());
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! Construction performs no request or transport activity. Call rmcp's
//! `ServiceExt::serve` with the transport you choose when ready to serve. The
//! repository's `examples/server.rs` supplies the complete stdio lifecycle.
//!
//! # Custom integration
//!
//! Keep your own `ServerHandler` when you need authorization middleware, prompts,
//! unrelated resources or custom lifecycle behavior. Prepare the same router,
//! call its validation before serving, then delegate with the original request
//! and context. Its lists are unpaginated; your handler owns filtering, pagination
//! and selecting other resource owners. Callback errors are not fallback signals.
//! See `AppRouter` for executable tool and dynamic-resource examples and
//! `AppServer` for the supported ready-server methods. The repository's
//! `examples/custom_server.rs` implements the complete manual equivalent.
//!
//! <details>
//! <summary>Implement your own handler over the same prepared router</summary>
//!
//! This handler replaces the ready adapter. With macros, obtain the same router
//! from `Inventory::app_router()`; with manual declarations, register resources
//! and associate rmcp tools explicitly. Validate the router and
//! store it with your application state. These methods let you insert access
//! checks and retain other MCP methods. A fully manual resource declaration is
//! demonstrated on [`StaticUiResource`].
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "server")] {
//! use mcp_apps_server::{AppRouter, UiResource, UiResourceRoute, UiResourceUri};
//! use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
//!
//! struct ExistingServer<S> { state: S, apps: AppRouter<S> }
//! impl<S: Send + Sync + 'static> ServerHandler for ExistingServer<S> {
//!     fn get_info(&self) -> ServerConfig {
//!         let mut info = ServerConfig::default();
//!         self.apps.declare_capabilities(&mut info.capabilities);
//!         info
//!     }
//!     fn get_tool(&self, name: &str) -> Option<Tool> { self.apps.get_tool(name) }
//!     async fn list_tools(&self, request: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>)
//!         -> Result<ListToolsResult, ErrorData> { self.apps.list_tools(request) }
//!     async fn list_resources(&self, request: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>)
//!         -> Result<ListResourcesResult, ErrorData> { self.apps.list_resources(request) }
//!     async fn call_tool(&self, request: CallToolRequestParams, context: RequestContext<RoleServer>)
//!         -> Result<CallToolResponse, ErrorData> {
//!         // Authorize the original request before delegating.
//!         self.apps.call_tool(&self.state, request, context).await
//!     }
//!     async fn read_resource(&self, request: ReadResourceRequestParams, context: RequestContext<RoleServer>)
//!         -> Result<ReadResourceResponse, ErrorData> {
//!         // Select other resource owners by URI here when needed.
//!         self.apps.read_resource(&self.state, request, context).await
//!     }
//! }
//! # let mut apps = AppRouter::<()>::new();
//! # apps.register_resource(UiResourceRoute::static_html(
//! #     UiResource::new(UiResourceUri::new("ui://inventory/dashboard.html")?, "dashboard"),
//! #     "<!doctype html><html><body>Inventory</body></html>"))?;
//! # apps.validate()?;
//! # let server = ExistingServer { state: (), apps };
//! # assert!(server.get_info().capabilities.resources.is_some());
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! </details>
//!
//! # Low-level declarations
//!
//! [`UiResource`] prepares a descriptor and matching HTML contents. [`ToolUi`]
//! and [`ResourceUi`] prepare typed declarations and compose them into open rmcp
//! metadata, preserving unknown keys. Omitted fields leave existing values intact;
//! unequal supplied scalar/array values return [`MetadataError::Conflict`] without
//! a partial update. These primitives remain usable without either optional feature.
//!
//! ```
//! use mcp_apps_server::{ResourceUi, UiResource, UiResourceUri};
//! let resource = UiResource::new(UiResourceUri::new("ui://inventory/dashboard")?, "dashboard");
//! let mut ui = ResourceUi::default();
//! ui.prefers_border = Some(true);
//! let contents = resource.html_contents("<!doctype html><html><body>Inventory</body></html>")
//!     .with_meta(ui.to_meta()?);
//! # let _ = contents;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Capabilities, security and limits
//!
//! [`ui_capability`] inspects caller-supplied client capabilities and distinguishes
//! absent, malformed and valid non-HTML support. On current rmcp requests use
//! `RequestContext::client_capabilities()` rather than assume initialization is
//! always the source. The consuming application chooses its fallback behavior.
//!
//! HTML is supplied by the consumer, without parsing, sanitization or bundling.
//! CSP, permissions and visibility are declarations for the host, not access
//! control implemented by the server. The browser renders the interface and the
//! host mediates communication. Local protocol checks do not establish real-host
//! rendering or permission behavior. No runtime, transport or background work is
//! started by this crate.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod capabilities;
mod metadata;
mod resource;
#[cfg(feature = "server")]
mod router;
#[cfg(feature = "server")]
mod server;
mod ui;
mod uri;

pub use capabilities::{CapabilityError, UiCapability, declare_ui_extension, ui_capability};
/// Derive a static resource's declaration, embedded HTML and optional metadata.
///
/// Requires `macros` (or its compatibility alias `derive`). Supply `name`, either
/// `id` or `uri`, and either `html_file` or `html` as string literals in
/// `#[ui_resource(...)]`. `id` is a nonempty logical identifier without `ui://`;
/// that prefix is added and the resulting URI is validated. `uri` supplies a
/// complete URI. Neither form requires an `.html` suffix or derives identity from
/// the filesystem. `html` retains the original file-path meaning of `html_file`.
/// Supplying both alternatives of a pair is a compilation error.
/// Optional `description` is a string; `ui` is a function path returning
/// [`ResourceUi`], `meta` is a function path returning `Result<MetaObject,
/// MetadataError>`, and `crate` is the path of a renamed dependency. The complete
/// open map from `meta` is composed with `ui` at registration; omitted options
/// retain trait defaults. See [`StaticUiResource::meta`] for extension ownership,
/// preparation errors and the placement of returned annotations.
/// The compiler includes the UTF-8 HTML file relative to the declaring Rust file.
/// Nothing is read from the filesystem by the running server. Generic struct
/// parameters and bounds are retained without adding field-trait requirements.
/// For a fixed view, prefer a fieldless struct. Its type identifies the resource;
/// registration never constructs an instance or reads any struct fields.
///
/// Invalid literal URIs are compilation errors, using the runtime's parser:
///
/// ```compile_fail
/// use mcp_apps_server::StaticUiResource;
/// #[derive(StaticUiResource)]
/// #[ui_resource(uri = "https://example.com/view", name = "view", html = "../examples/dashboard.html")]
/// struct InvalidView;
/// ```
#[cfg(feature = "macros")]
pub use mcp_apps_server_macros::StaticUiResource;
/// Declare an application's rmcp tools and their static UI resources together.
///
/// Requires `macros` and `server`; the consumer enables rmcp's `macros` feature.
/// Apply this to an inherent impl with directly annotated rmcp `#[tool]` methods.
/// Mark UI tools with `#[ui(resource = ResourceType)]`; the type must implement
/// [`StaticUiResource`]. Unmarked tools keep their original declarations.
/// Do not also apply rmcp's `tool_router` to this block.
///
/// The generated public `app_router()` method prepares an [`AppRouter`] and returns
/// registration errors before serving. It registers resources first, composes each
/// association into rmcp's original tool metadata, then registers checked routes.
/// Custom rmcp tool names, schemas, annotations and callbacks are retained. Several
/// tools can share a URI only with identical descriptors, HTML and content metadata.
/// Metadata conflicts and duplicate tool names fail rather than overwrite routes.
/// A failed preparation returns no router; tool handlers are never called by it.
/// Resource declaration methods and rmcp metadata factories must have no side effects.
///
/// Use `#[app_router(crate = apps_sdk)]` for a renamed runtime dependency. Generic
/// impl bounds are preserved; router preparation requires `Self: Sized + Send +
/// Sync + 'static`. Method `cfg` and gating `cfg_attr` attributes also
/// gate registration; put `tool` and `ui` directly on the method, outside `cfg_attr`.
/// Helper methods remain unchanged; the method name `app_router` is reserved.
/// The macro does not implement `ServerHandler`, start a transport or load assets.
/// Pass its router to [`AppServer::new`] or invoke it from your custom handler.
/// See the crate's getting-started example for a complete typed workflow.
#[cfg(all(feature = "macros", feature = "server"))]
pub use mcp_apps_server_macros::app_router;
pub use metadata::MetadataError;
pub use resource::{StaticUiResource, UiResource};
/// An open MCP metadata map, reused from rmcp for extension annotations.
///
/// This is the same type as [`rmcp::model::MetaObject`]; its keys and values are
/// owned by the extension that defines them. See [`StaticUiResource::meta`] for
/// composition with generic MCP Apps resource settings.
pub use rmcp::model::MetaObject;
#[cfg(feature = "server")]
pub use router::{AppRouter, HtmlFuture, RegistrationError, UiResourceRoute};
#[cfg(feature = "server")]
pub use server::AppServer;
pub use ui::{PermissionRequest, ResourceUi, ToolUi, UiCsp, UiPermissions, UiVisibility};
pub use uri::{UiResourceUri, UiUriError};

/// The MCP Apps HTML MIME type required on returned resource contents.
pub const UI_MIME_TYPE: &str = "text/html;profile=mcp-app";

/// The standard MCP Apps capability extension identifier.
pub const UI_EXTENSION_ID: &str = "io.modelcontextprotocol/ui";
