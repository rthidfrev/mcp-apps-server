# MCP Apps Server Macros

Alpha `0.1.0-alpha.1`: optional procedural macros for the
[`mcp-apps-server` Rust library](https://crates.io/crates/mcp-apps-server).
Consumers normally use the library's reexports rather than depend on this package
directly.
Select the alpha through the runtime library's reexports:

```toml
[dependencies]
mcp-apps-server = { version = "=0.1.0-alpha.1", features = ["server", "macros"] }
```

- `StaticUiResource` derives a fixed UI resource declaration, with a logical
  identifier or full UI URI and HTML embedded from a source-relative file.
- `app_router` associates rmcp tools with those resource types and generates
  checked application registration. It requires the runtime library's `server`
  feature; the consuming server separately enables rmcp's tool macros.

The consumer's compiler executes the macros. They generate implementations using
the runtime library's validation and routing; they do not start a server, invoke
tools or render a browser interface. Resource metadata supplied by another
extension remains that extension SDK's responsibility.

The [macro API reference](https://docs.rs/mcp-apps-server-macros/0.1.0-alpha.1/mcp_apps_server_macros/)
documents each macro's declaration contract, attributes and errors. Both packages
support Rust 1.88. Project-owned code is MIT licensed; the license text is included
in this package.
