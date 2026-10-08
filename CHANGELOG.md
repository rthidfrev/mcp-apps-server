# Changelog

## [0.1.0-alpha.1](https://github.com/rthidfrev/mcp-apps-server/tree/v0.1.0-alpha.1) - 2026-10-08

Initial alpha of `mcp-apps-server` and its optional `mcp-apps-server-macros` package.

- Validated UI resource identifiers, typed MCP Apps metadata and capability helpers.
- Application registration, checked tool/view associations and static or dynamic
  HTML resource routing with rmcp.
- A ready `AppServer` adapter and the same `AppRouter` for custom server handlers.
- Optional `StaticUiResource` and `app_router` macros using the checked runtime API.
- Runnable server examples, executable API documentation and Rust 1.88 support.

The API may change between alpha candidates. The library serves consumer-supplied
HTML; the browser and host own rendering, communication and sandbox policy.
Consumers retain authorization, application services and transport lifecycle.
