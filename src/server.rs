use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};

use crate::{AppRouter, RegistrationError};

/// A ready-to-serve MCP server for a prepared application router and its state.
///
/// Requires the `server` feature. The router supplies tools and UI resources;
/// rmcp supplies protocol handling. The consumer supplies the state, runtime and
/// transport, and performs authorization inside tool/resource callbacks.
///
/// This adapter implements tool/resource listing, tool execution, resource reads
/// and matching capabilities. For prompts, tasks, custom notifications, filtered
/// listings or an existing server's policies, invoke [`AppRouter`] from your own
/// [`ServerHandler`] instead. It does not wrap or forward an existing handler.
/// Routes and declarations are fixed at construction; no list-change capability
/// or resource subscription is advertised.
pub struct AppServer<S> {
    state: S,
    router: AppRouter<S>,
    info: ServerConfig,
}

impl<S: Send + Sync + 'static> AppServer<S> {
    /// Validate all associations and construct the server with matching capabilities.
    ///
    /// Returns a [`RegistrationError`] if a UI tool refers to an unregistered or
    /// invalid resource. No request is executed and no transport is started.
    pub fn new(state: S, router: AppRouter<S>) -> Result<Self, RegistrationError> {
        router.validate()?;
        let mut info = ServerConfig::default();
        router.declare_capabilities(&mut info.capabilities);
        Ok(Self {
            state,
            router,
            info,
        })
    }

    /// Set the consuming server's identity, rather than the library's identity.
    pub fn with_server_info(mut self, implementation: Implementation) -> Self {
        self.info.server_info = implementation;
        self
    }

    /// Add instructions that rmcp returns with this server's information.
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.info.instructions = Some(instructions.into());
        self
    }

    /// Borrow the consumer state used by tool and resource callbacks.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Borrow the prepared router without exposing mutable registration state.
    pub fn router(&self) -> &AppRouter<S> {
        &self.router
    }
}

impl<S: Send + Sync + 'static> ServerHandler for AppServer<S> {
    fn get_info(&self) -> ServerConfig {
        self.info.clone()
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

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.router.call_tool(&self.state, request, context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.router.list_resources(request)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.router
            .read_resource(&self.state, request, context)
            .await
    }
}
