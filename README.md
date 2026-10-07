# MCP Apps Server

A Rust SDK for registering [MCP Apps](https://modelcontextprotocol.io/extensions/apps/overview)
with `rmcp`. Declare typed tools and HTML resources, associate them once, and use
a ready server or the same router inside your existing server.

> **Alpha: `0.1.0-alpha.1`.** Tested manually on **ChatGPT Desktop 26.930.61225**
> only. Compatibility with other hosts and exhaustive feature coverage have not
> been established. The API may change during alpha development. Evaluate it in
> your integration before relying on it.

Requires Rust **1.88** or newer. MIT licensed.

## Choose your integration

| Need | Entry point |
| --- | --- |
| Serve an application with typed tools and embedded HTML | `StaticUiResource` + `app_router` macros, then `AppServer` |
| Keep custom authorization, prompts, resources or lifecycle | Invoke `AppRouter` from your own `ServerHandler` |
| Compute or authorize HTML on each read | `UiResourceRoute::new` callback |
| Construct protocol declarations yourself | `UiResource`, `ToolUi`, `ResourceUi`, URI and capability helpers |

The examples below describe the same inventory application. The
[complete simple example](examples/server.rs) and
[custom handler example](examples/custom_server.rs) share their
[typed models and snapshot logic](examples/support/mod.rs).

## Getting started

Add the alpha release to your consuming server's `Cargo.toml`. Pin the candidate
explicitly: API compatibility between alpha candidates is not promised. Select
rmcp's macros and transport separately:

```toml
[dependencies]
mcp-apps-server = { version = "=0.1.0-alpha.1", features = ["server", "macros"] }
rmcp = { version = "3.4.1", default-features = false, features = ["server", "macros", "transport-io"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt", "io-util"] }
```

This complete server embeds an `inventory.html` file next to its Rust source. The
compiler reads that file; the running server performs no filesystem load for it.
rmcp generates input/output schemas from the Rust models and extracts tool inputs.

```rust
use mcp_apps_server::{AppServer, StaticUiResource, app_router};
use rmcp::{
    ServiceExt, model::Implementation, schemars::JsonSchema, tool,
    handler::server::wrapper::{Json, Parameters},
};
use serde::{Deserialize, Serialize};

#[derive(StaticUiResource)]
#[ui_resource(id = "inventory/dashboard", name = "inventory-dashboard", html_file = "inventory.html")]
struct Dashboard;

#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
struct SnapshotParams { sku: String }

#[derive(Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct Snapshot { sku: String, available: u32, reserved: u32 }

struct Inventory;

#[app_router]
impl Inventory {
    #[tool(description = "Read the inventory for an item")]
    #[ui(resource = Dashboard)]
    fn inventory_snapshot(&self, Parameters(input): Parameters<SnapshotParams>) -> Json<Snapshot> {
        Json(Snapshot { sku: input.sku, available: 12, reserved: 3 })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppServer::new(Inventory, Inventory::app_router()?)?
        .with_server_info(Implementation::new("inventory", "0.1.0"))
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
```

`id = "inventory/dashboard"` becomes `ui://inventory/dashboard`. It is a logical
identifier, independent of the HTML path, and needs no `.html` suffix. `html_file`
is relative to the declaring Rust source. The fieldless `Dashboard` type identifies
one fixed view; no instance is constructed. `uri` and `html` are alternatives to
`id` and `html_file`; use exactly one option from each pair.

`#[ui(resource = Dashboard)]` expresses the tool/view relationship once. The
generated `Inventory::app_router()` registers the view and composes its URI into
RMCP's tool metadata, including custom `#[tool(name = "...")]` names. Ordinary
tools need only `#[tool]`. Several tools can share a view when its descriptor,
HTML and resource metadata agree; conflicting declarations fail. Keep resource
declarations and tool metadata factories free of side effects.

Registration checks resource identity and associations. `AppServer` validates the
assembled router, advertises its tool/resource/UI capabilities and handles their
MCP requests. Your tool still owns business logic and access checks. The consuming
program starts the runtime and transport; stdout is reserved for MCP messages.
`Json<Snapshot>` provides structured output and ordinary text content, including
for clients that do not advertise UI support.

<details>
<summary>Add resource metadata from another extension SDK</summary>

Keep that extension's Rust types and validation in its SDK. The static declaration
accepts `meta = function_path`, a function returning `Result<MetaObject,
MetadataError>`. The following illustrative display preference uses an
application-owned namespace; it does not define a host protocol:

```rust
use mcp_apps_server::{MetaObject, MetadataError, ResourceUi, StaticUiResource};
use serde::Serialize;

#[derive(Serialize)]
struct DisplayPreferences { compact: bool }

fn dashboard_meta() -> Result<MetaObject, MetadataError> {
    let mut meta = MetaObject::default();
    meta.0.insert("example/display".into(),
        serde_json::to_value(DisplayPreferences { compact: true })?);
    Ok(meta)
}

fn dashboard_ui() -> ResourceUi {
    let mut ui = ResourceUi::default();
    ui.prefers_border = Some(true);
    ui
}

#[derive(StaticUiResource)]
#[ui_resource(id = "inventory/dashboard", name = "inventory-dashboard",
    html_file = "inventory.html", ui = dashboard_ui, meta = dashboard_meta)]
struct Dashboard;
```

This example also needs `serde_json = "1"` in the consumer's dependencies. Use
your extension SDK's preparation operation in `dashboard_meta`; if it returns its
own error, preserve it with `.map_err(|error|
MetadataError::Preparation(Box::new(error)))?`.

The handwritten equivalent overrides
`fn meta() -> Result<MetaObject, MetadataError>` on `StaticUiResource`. Omitting
the method or derive option keeps the empty-map default. Both `register_static`
and the generated `app_router()` compose this map with `ui()` and preserve
unrelated keys, including nested data. A non-object root `ui`, contradictory
MCP Apps settings or a preparation failure stops registration without inserting
the resource. Shared views must agree on the complete composed metadata too.

Annotations accompany the HTML in `resources/read` contents, not the resource
listing. The consuming SDK defines their meaning; the host decides whether to
honor display preferences. This library neither interprets them nor enforces
host behavior. The [paired runnable examples](examples/support/mod.rs) show
the same metadata factory in both registration styles.

</details>

<details>
<summary>Keep explicit resource declarations and your own ServerHandler</summary>

Disable `macros` and `derive` if you do not need them. The handwritten implementation
describes the same resource; `ResourceUi` can supply CSP, permissions and other
settings through its `ui()` method in either form.

```rust
use mcp_apps_server::{StaticUiResource, UiResource, UiResourceUri, UiUriError};

struct Dashboard;
impl StaticUiResource for Dashboard {
    fn resource() -> Result<UiResource, UiUriError> {
        Ok(UiResource::new(
            UiResourceUri::new("ui://inventory/dashboard")?,
            "inventory-dashboard",
        ))
    }
    fn html() -> &'static str { include_str!("inventory.html") }
}
```

Reuse the input/output models from the simple example. Replace its annotated impl
with RMCP's `tool_router` and register the resource and association explicitly:

```rust
use mcp_apps_server::AppRouter;

#[rmcp::tool_router]
impl Inventory {
    #[rmcp::tool(description = "Read the inventory for an item")]
    fn inventory_snapshot(&self, Parameters(input): Parameters<SnapshotParams>) -> Json<Snapshot> {
        Json(Snapshot { sku: input.sku, available: 12, reserved: 3 })
    }
}

let mut apps = AppRouter::from_tool_router(Inventory::tool_router());
apps.register_static::<Dashboard>()?;
apps.associate_static_tool::<Dashboard>("inventory_snapshot")?;
apps.validate()?;
```

Alternatively, keep `app_router` to prepare this same router and change only the
server handler. Your handler can authorize requests before delegation and handle
prompts, unrelated resources, notifications and lifecycle itself. Pass the
original request and context to the router:

```rust
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
struct ExistingServer {
    inventory: Inventory,
    apps: AppRouter<Inventory>,
}

impl ServerHandler for ExistingServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::default();
        self.apps.declare_capabilities(&mut info.capabilities);
        info
    }

    fn get_tool(&self, name: &str) -> Option<Tool> { self.apps.get_tool(name) }

    async fn list_tools(&self, request: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>)
        -> Result<ListToolsResult, ErrorData> {
        self.apps.list_tools(request)
    }

    async fn call_tool(&self, request: CallToolRequestParams, context: RequestContext<RoleServer>)
        -> Result<CallToolResponse, ErrorData> {
        // Run your request authorization here, before delegating.
        self.apps.call_tool(&self.inventory, request, context).await
    }

    async fn list_resources(&self, request: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>)
        -> Result<ListResourcesResult, ErrorData> {
        self.apps.list_resources(request)
    }

    async fn read_resource(&self, request: ReadResourceRequestParams, context: RequestContext<RoleServer>)
        -> Result<ReadResourceResponse, ErrorData> {
        // For additional resources, choose the owner by URI before dispatch.
        self.apps.read_resource(&self.inventory, request, context).await
    }
}
```

The [runnable custom server](examples/custom_server.rs) supplies construction and
stdio lifecycle. The router passes callback errors through; authorization failure
must not be treated as a reason to try a different resource handler. Its lists
are unpaginated and reject cursors. A custom server owns filtered listings and
pagination when combining them with other sources.

</details>

<details>
<summary>Load or authorize HTML per request, and retain custom metadata</summary>

Dynamic routes use the same registry and URI/MIME guarantees. The callback runs
only for a matching read and receives the original request context. Return an MCP
error for a denied or failed read; do not put side effects in resource registration.

```rust
use mcp_apps_server::{AppRouter, ResourceUi, UiResource, UiResourceRoute, UiResourceUri};

struct Inventory { dashboard_html: String }
fn dynamic_router() -> Result<AppRouter<Inventory>, Box<dyn std::error::Error>> {
    let resource = UiResource::new(UiResourceUri::new("ui://inventory/dashboard.html")?, "dashboard");
    let route = UiResourceRoute::new(resource, |state: &Inventory, context| {
        Box::pin(async move {
            // Use context metadata/extensions for your application's access checks.
            let _request_id = context.id;
            Ok(state.dashboard_html.clone())
        })
    }).with_ui(&ResourceUi::default())?;
    let mut apps = AppRouter::new();
    apps.register_resource(route)?;
    Ok(apps)
}
```

`with_meta` accepts a complete open metadata map; `with_ui` then composes typed UI
settings while preserving unknown keys. Unequal supplied scalar/array values are
conflicts, with no partial update. Metadata is returned on HTML content. CSP and
permissions describe host policy; the consuming server still enforces access.

</details>

## Features and API navigation

| Feature | Added capability |
| --- | --- |
| Default / no features | Validated URI, typed metadata, resource/capability helpers and handwritten `StaticUiResource` |
| `server` | `AppRouter`, `AppServer`, `UiResourceRoute` and registration errors; enables rmcp server support |
| `macros` | `StaticUiResource` derive; with `server`, also `app_router`. Does not enable rmcp tool macros or a transport |
| `derive` | Alias for `macros` |

Features are additive and disabled by default. The macro delegates to the same
checked runtime API. Its `ui = function_path` option supplies typed resource
settings and `meta = function_path` supplies the complete open content metadata;
`crate = renamed_dependency` supports a renamed runtime dependency.
`app_router` also accepts `crate = renamed_dependency`. Put `tool` and `ui` directly
on methods; `cfg` and gating `cfg_attr` conditions govern their registration too.
The generated `app_router` method requires send-capable, static server state. Use
the manual API for dynamic views or registration assembled in other modules.
Several tools can share one resource, and one router can serve several interfaces.
Use checked `merge` for separately assembled application routers; duplicate tool
names, exact resource URIs or inconsistent imported tool names fail before changing
the destination. Source disabled-name markers cannot hide existing destination
tools either. UI associations are validated on finalization, so resources can
come from another assembled router. Names already overwritten while building an
rmcp router cannot be recovered afterward.

The [crate documentation](src/lib.rs) provides executable workflows and links to
item contracts. docs.rs builds and hosts the
[API reference](https://docs.rs/mcp-apps-server/0.1.0-alpha.1/mcp_apps_server/)
with all features enabled. To generate the reference from a checkout, run this
command at the repository root:

```sh
cargo doc --locked --workspace --no-deps --all-features --open
```

Open `target/doc/mcp_apps_server/index.html` for the API reference.

## Run and verify

Run these commands from the repository root:

```sh
cargo run --locked --example server --features server,macros
cargo run --locked --example custom_server --features server
```

Both commands start a server using the standard input/output transport. Connect
an MCP client to call the inventory tool and read its
[dashboard HTML](examples/dashboard.html). For browser interactions, use the
[official MCP Apps browser SDK](https://github.com/modelcontextprotocol/ext-apps/tree/v2.0.3/examples).
The library registers and serves HTML; the browser renders it and the host mediates
communication and sandbox policy. The consuming server owns authorization, services
and transport lifecycle; UI visibility, CSP and permissions do not replace its
access checks.

Automated tests cover URI and metadata validation, registration, typed tool
schemas/results, and resource routing through in-memory MCP connections.

```sh
cargo check --locked --lib --no-default-features
cargo test --locked --workspace --all-features
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +1.88.0 test --locked --workspace --all-features
```

Set `RUSTDOCFLAGS="-D warnings"` when generating documentation to treat rustdoc
warnings as errors. In-memory MCP tests do not cover rendering or interaction
inside a host.

The [GitHub Actions workflow](.github/workflows/ci.yml) runs on pushes to `main`,
pull requests and manual dispatch. GitHub runs the tests on Linux and Windows
with stable Rust, and on Linux with Rust 1.88. A separate Linux job checks
formatting, strict Clippy, the minimal and individual feature configurations,
strict API documentation and both Cargo archives. These checks do not publish
packages or exercise a browser host.

## Contributing and license

See [AGENTS.md](AGENTS.md) for contribution guidelines and
[RELEASING.md](RELEASING.md) for compatibility and release preparation.
Version changes are recorded in [CHANGELOG.md](CHANGELOG.md).
Licensed under [MIT](LICENSE-MIT).
