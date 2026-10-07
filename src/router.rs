//! Register rmcp tools and matching HTML resources without owning a transport.
//!
//! Routes use send-capable callbacks. A custom [`rmcp::ServerHandler`] can invoke
//! this router after its own authorization and filtering. Call
//! [`AppRouter::validate`] before serving when importing a [`ToolRouter`]; an
//! imported tool may refer to a resource registered later. The router does not
//! start a runtime, load assets, enforce UI visibility or render a browser app.
//!
//! # Register an existing rmcp tool
//!
//! ```
//! use mcp_apps_server::{AppRouter, ToolUi, UiResource, UiResourceRoute, UiResourceUri};
//! use rmcp::{tool, tool_router};
//!
//! struct Inventory;
//! #[tool_router]
//! impl Inventory {
//!     #[tool(description = "Read the inventory snapshot")]
//!     fn snapshot(&self) -> String {
//!         "12 items available".into()
//!     }
//! }
//!
//! let view = UiResource::new(UiResourceUri::new("ui://inventory/view")?, "inventory");
//! let ui = ToolUi::new(view.uri().clone());
//! let mut router = AppRouter::from_tool_router(Inventory::tool_router());
//! router.register_resource(UiResourceRoute::static_html(
//!     view, "<!doctype html><html><body>Inventory</body></html>",
//! ))?;
//! router.associate_tool("snapshot", &ui)?;
//! router.validate()?;
//! assert_eq!(router.list_tools(None)?.tools.len(), 1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{collections::BTreeMap, error::Error, fmt, future::Future, pin::Pin};

use rmcp::{
    ErrorData, RoleServer,
    handler::server::{
        router::tool::{IntoToolRoute, ToolRouter},
        tool::ToolCallContext,
    },
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, ListResourcesResult, ListToolsResult,
        MetaObject, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse,
        ReadResourceResult, ServerCapabilities, Tool,
    },
    service::RequestContext,
};

use crate::{
    MetadataError, ResourceUi, StaticUiResource, ToolUi, UiResource, UiResourceUri, UiUriError,
    declare_ui_extension,
};

/// The send-capable result of an HTML resource callback.
///
/// The callback may borrow application state until it finishes. Its error is
/// returned unchanged; the router adds the registered URI, MIME type and metadata
/// only to a successful HTML result.
pub type HtmlFuture<'a> = Pin<Box<dyn Future<Output = Result<String, ErrorData>> + Send + 'a>>;

type HtmlHandler<S> =
    dyn for<'a> Fn(&'a S, RequestContext<RoleServer>) -> HtmlFuture<'a> + Send + Sync;

enum HtmlSource<S> {
    Static(String),
    Callback(Box<HtmlHandler<S>>),
}

/// A registered HTML source with fixed identity and resource metadata.
///
/// Dynamic callbacks receive the application state and original request context,
/// including cancellation, request metadata and transport extensions. They run
/// only for reads matching the registered URI. They supply HTML, not a protocol
/// resource object, so cannot accidentally change its URI or MIME type.
/// The consuming application owns authorization and HTML validity.
pub struct UiResourceRoute<S> {
    resource: UiResource,
    source: HtmlSource<S>,
    meta: MetaObject,
}

impl<S> UiResourceRoute<S> {
    fn from_static<R: StaticUiResource>() -> Result<Self, RegistrationError> {
        Ok(Self::static_html(R::resource()?, R::html())
            .with_meta(R::meta()?)?
            .with_ui(&R::ui())?)
    }

    /// Prepare a dynamic HTML callback without executing it.
    ///
    /// No file or network access occurs during registration. The callback receives
    /// the request context when [`AppRouter::read_resource`] dispatches a matching
    /// read, and may use its extensions for application-specific access checks.
    ///
    /// ```
    /// use mcp_apps_server::{UiResource, UiResourceRoute, UiResourceUri};
    ///
    /// struct Application { html: String }
    /// let resource = UiResource::new(UiResourceUri::new("ui://application/view")?, "view");
    /// let route = UiResourceRoute::new(resource, |state: &Application, context| {
    ///     Box::pin(async move {
    ///         // Application-specific access checks can inspect context.extensions.
    ///         if context.ct.is_cancelled() {
    ///             return Err(rmcp::ErrorData::internal_error("Read was cancelled", None));
    ///         }
    ///         Ok(state.html.clone())
    ///     })
    /// });
    /// # let _ = route;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(
        resource: UiResource,
        callback: impl for<'a> Fn(&'a S, RequestContext<RoleServer>) -> HtmlFuture<'a>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            resource,
            source: HtmlSource::Callback(Box::new(callback)),
            meta: MetaObject::default(),
        }
    }

    /// Prepare caller-supplied static HTML without loading or inspecting assets.
    ///
    /// Each successful read returns an owned copy of this HTML, as required by
    /// rmcp's resource-content representation.
    pub fn static_html(resource: UiResource, html: impl Into<String>) -> Self {
        Self {
            resource,
            source: HtmlSource::Static(html.into()),
            meta: MetaObject::default(),
        }
    }

    /// Replace the complete resource-content metadata map.
    ///
    /// Unknown keys remain available in the supplied map. Use [`Self::with_ui`]
    /// afterwards to compose a typed UI declaration without discarding them.
    /// Metadata is emitted on returned HTML content, not on the listing descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError::InvalidUi`] if a supplied root `ui` value is not
    /// an object. The foundation's serialization errors are also propagated.
    pub fn with_meta(mut self, mut meta: MetaObject) -> Result<Self, MetadataError> {
        ResourceUi::default().merge_into(&mut meta)?;
        self.meta = meta;
        Ok(self)
    }

    /// Compose resource UI settings with the current content metadata.
    ///
    /// # Errors
    ///
    /// Returns the same atomic validation and conflict errors as
    /// [`ResourceUi::merge_into`]. It does not validate HTML or host-specific CSP
    /// domain syntax, grant permissions or change the resource's identity.
    pub fn with_ui(mut self, ui: &ResourceUi) -> Result<Self, MetadataError> {
        ui.merge_into(&mut self.meta)?;
        Ok(self)
    }
}

/// A registration or finalization failure, before serving requests.
///
/// Failed registration, association and merge operations leave the destination
/// router unchanged. URI and metadata errors preserve their original causes.
#[derive(Debug)]
#[non_exhaustive]
pub enum RegistrationError {
    /// A tool name is already registered, even if its route is disabled.
    DuplicateTool {
        /// The conflicting tool name.
        name: String,
    },
    /// An imported rmcp map key disagrees with its tool's declared name.
    ToolNameMismatch {
        /// The name used by rmcp for lookup and dispatch.
        registered_name: String,
        /// The name emitted in the tool declaration.
        declared_name: String,
    },
    /// A resource with this exact URI spelling is already registered.
    DuplicateResource {
        /// The conflicting resource URI.
        uri: String,
    },
    /// A shared static URI was declared with different content or metadata.
    ConflictingResource {
        /// The URI whose existing registration must not be replaced.
        uri: String,
    },
    /// The requested tool association has no registered tool.
    UnknownTool {
        /// The missing tool name.
        name: String,
    },
    /// An explicit association omitted its required resource URI.
    MissingResourceUri {
        /// The tool that was to be associated.
        tool: String,
    },
    /// An association refers to an HTML resource absent from this router.
    UnregisteredResource {
        /// The missing resource URI.
        uri: String,
    },
    /// An existing tool's URI association is not a JSON string.
    InvalidToolAssociation {
        /// The tool carrying the malformed declaration.
        tool: String,
        /// The malformed protocol field.
        path: String,
    },
    /// A supplied UI resource URI failed validation.
    Uri(UiUriError),
    /// UI metadata could not be validated or composed.
    Metadata(MetadataError),
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTool { name } => {
                write!(formatter, "tool {name:?} is already registered")
            }
            Self::ToolNameMismatch {
                registered_name,
                declared_name,
            } => write!(
                formatter,
                "registered tool name {registered_name:?} differs from declaration {declared_name:?}"
            ),
            Self::DuplicateResource { uri } => {
                write!(formatter, "resource {uri:?} is already registered")
            }
            Self::ConflictingResource { uri } => {
                write!(
                    formatter,
                    "resource {uri:?} has a conflicting static declaration"
                )
            }
            Self::UnknownTool { name } => write!(formatter, "tool {name:?} is not registered"),
            Self::MissingResourceUri { tool } => {
                write!(
                    formatter,
                    "association for tool {tool:?} requires a resource URI"
                )
            }
            Self::UnregisteredResource { uri } => {
                write!(formatter, "associated resource {uri:?} is not registered")
            }
            Self::InvalidToolAssociation { tool, path } => {
                write!(formatter, "tool {tool:?} requires a string at {path}")
            }
            Self::Uri(error) => write!(formatter, "invalid registered UI URI: {error}"),
            Self::Metadata(error) => write!(formatter, "invalid registration metadata: {error}"),
        }
    }
}

impl Error for RegistrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            Self::Metadata(error) => Some(error),
            _ => None,
        }
    }
}

impl From<UiUriError> for RegistrationError {
    fn from(error: UiUriError) -> Self {
        Self::Uri(error)
    }
}

impl From<MetadataError> for RegistrationError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

/// An rmcp tool router and exact-URI registry of HTML resources.
///
/// Use this directly in a custom [`rmcp::ServerHandler`] when that handler owns
/// authorization, other resources, prompts, tasks or custom lifecycle behavior.
/// Request dispatch uses the existing rmcp tool framework. The application state
/// and callbacks must be `Send + Sync`; this API does not promise rmcp's relaxed
/// `local` callback support. Imported tools require [`Self::validate`] after all
/// resources have been registered and before serving.
pub struct AppRouter<S> {
    tools: ToolRouter<S>,
    resources: BTreeMap<String, UiResourceRoute<S>>,
}

impl<S> Default for AppRouter<S> {
    fn default() -> Self {
        Self {
            tools: ToolRouter::default(),
            resources: BTreeMap::new(),
        }
    }
}

impl<S: Send + Sync + 'static> AppRouter<S> {
    /// Construct an empty registration set without performing external effects.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adopt rmcp routes, including their disabled markers and notifier.
    ///
    /// This does not validate their existing UI associations: register the
    /// corresponding resources and call [`Self::validate`] before serving.
    /// Duplicate names already overwritten by rmcp cannot be recovered here.
    pub fn from_tool_router(tools: ToolRouter<S>) -> Self {
        Self {
            tools,
            resources: BTreeMap::new(),
        }
    }

    /// Register a static or dynamic resource by its exact validated URI.
    ///
    /// The callback is not executed. No name restrictions, normalization, asset
    /// loading or HTML validation are introduced.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::DuplicateResource`] without replacing the
    /// existing route when the URI is already registered.
    pub fn register_resource(
        &mut self,
        route: UiResourceRoute<S>,
    ) -> Result<(), RegistrationError> {
        let uri = route.resource.uri().to_string();
        if self.resources.contains_key(&uri) {
            return Err(RegistrationError::DuplicateResource { uri });
        }
        self.resources.insert(uri, route);
        Ok(())
    }

    /// Register a statically defined UI resource and its complete content metadata.
    ///
    /// Composes [`StaticUiResource::ui`] with [`StaticUiResource::meta`], preserving
    /// unrelated extension data and rejecting conflicting declarations.
    ///
    /// # Errors
    ///
    /// Propagates URI and metadata errors from [`StaticUiResource`], or returns
    /// [`RegistrationError::DuplicateResource`] for an existing URI. Nothing is
    /// inserted until the complete definition has been prepared successfully.
    pub fn register_static<R: StaticUiResource>(&mut self) -> Result<(), RegistrationError> {
        let route = UiResourceRoute::from_static::<R>()?;
        self.register_resource(route)
    }

    /// Register or reuse an identical static declaration and return its URI.
    ///
    /// Application macros use this when several tools share a view. Reuse requires
    /// equal descriptors, HTML and prepared content metadata, even when different
    /// Rust types or type aliases declare the same URI. A dynamic route cannot be
    /// reused as a static declaration. No callback or filesystem access occurs.
    /// Open metadata and typed UI settings are composed as in [`Self::register_static`].
    /// Unlike [`Self::register_static`], an identical registration is accepted.
    ///
    /// # Errors
    ///
    /// Propagates URI and metadata errors while preparing the declaration, or
    /// returns [`RegistrationError::ConflictingResource`] if the existing route
    /// differs. Failure leaves the existing registration unchanged.
    pub fn ensure_static<R: StaticUiResource>(
        &mut self,
    ) -> Result<UiResourceUri, RegistrationError> {
        let route = UiResourceRoute::from_static::<R>()?;
        let uri = route.resource.uri().clone();
        if let Some(existing) = self.resources.get(uri.as_str()) {
            let same_html = match (&existing.source, &route.source) {
                (HtmlSource::Static(left), HtmlSource::Static(right)) => left == right,
                _ => false,
            };
            if !same_html
                || existing.resource.descriptor() != route.resource.descriptor()
                || existing.meta != route.meta
            {
                return Err(RegistrationError::ConflictingResource {
                    uri: uri.to_string(),
                });
            }
        } else {
            self.resources.insert(uri.to_string(), route);
        }
        Ok(uri)
    }

    /// Register an existing rmcp route without changing its handler or schema.
    ///
    /// Associated resources must already be registered. Tool factory functions,
    /// [`rmcp::handler::server::router::tool::ToolRoute`] and rmcp's other
    /// [`IntoToolRoute`] implementations are accepted directly.
    ///
    /// # Errors
    ///
    /// Rejects duplicate names, including disabled routes, and malformed,
    /// conflicting or unresolved UI associations before inserting anything.
    pub fn register_tool<R, A>(&mut self, route: R) -> Result<(), RegistrationError>
    where
        R: IntoToolRoute<S, A>,
    {
        let route = route.into_tool_route();
        if self.tools.map.contains_key(&route.attr.name) {
            return Err(RegistrationError::DuplicateTool {
                name: route.attr.name.to_string(),
            });
        }
        self.validate_tool(&route.attr)?;
        self.tools.add_route(route);
        Ok(())
    }

    /// Associate a registered tool with an already registered resource.
    ///
    /// Preserves its handler, schemas, annotations and unrelated metadata. A
    /// disabled route can be associated without re-enabling it. This operation
    /// declares UI visibility; it does not enforce access control.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::UnknownTool`],
    /// [`RegistrationError::MissingResourceUri`],
    /// [`RegistrationError::UnregisteredResource`] or the foundation's atomic
    /// metadata error, leaving the existing tool unchanged.
    pub fn associate_tool(&mut self, name: &str, ui: &ToolUi) -> Result<(), RegistrationError> {
        if !self.tools.map.contains_key(name) {
            return Err(RegistrationError::UnknownTool { name: name.into() });
        }
        let uri = ui
            .resource_uri
            .as_ref()
            .ok_or_else(|| RegistrationError::MissingResourceUri { tool: name.into() })?;
        if !self.contains_resource(uri.as_str()) {
            return Err(RegistrationError::UnregisteredResource {
                uri: uri.to_string(),
            });
        }
        let route = self
            .tools
            .map
            .get_mut(name)
            .ok_or_else(|| RegistrationError::UnknownTool { name: name.into() })?;
        ui.apply_to(&mut route.attr)?;
        Ok(())
    }

    /// Associate a tool with an already registered static resource definition.
    ///
    /// # Errors
    ///
    /// Propagates the definition's URI error and the same registration or metadata
    /// errors as [`Self::associate_tool`]. It does not register the resource.
    pub fn associate_static_tool<R: StaticUiResource>(
        &mut self,
        name: &str,
    ) -> Result<(), RegistrationError> {
        self.associate_tool(name, &ToolUi::new(R::resource()?.uri().clone()))
    }

    /// Merge another registration set after checking every name and URI collision.
    ///
    /// Both registration sets must have matching tool lookup keys and declared
    /// names. No destination route changes if an identity mismatch or collision
    /// exists, even when a resource collision is discovered after checking tools.
    /// Disabled routes still reserve their names. A source disabled-name marker
    /// without a route also conflicts with an existing destination tool; it
    /// cannot implicitly hide that tool. Destination markers for future tools
    /// and disabled source routes retain rmcp's behavior. The destination notifier
    /// is preserved; the source notifier is discarded, matching rmcp's merge
    /// behavior. No notification is emitted by this assembly operation. Validate
    /// the assembled associations before serving.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationError::ToolNameMismatch`],
    /// [`RegistrationError::DuplicateTool`] or
    /// [`RegistrationError::DuplicateResource`] without changing the destination.
    /// The source is consumed even on error.
    pub fn merge(&mut self, mut other: Self) -> Result<(), RegistrationError> {
        // rmcp reinserts routes by their declared name, discarding the original
        // lookup key. Check identities before that can hide an inconsistency.
        self.validate_tool_names()?;
        other.validate_tool_names()?;
        for name in other.tools.map.keys() {
            if self.tools.map.contains_key(name) {
                return Err(RegistrationError::DuplicateTool {
                    name: name.to_string(),
                });
            }
        }
        for name in self.tools.map.keys() {
            // After route collision checks, these names have no source route.
            // enable_route detects an orphan marker without emitting a notifier;
            // a marked source is consumed on this error, never merged.
            if other.tools.enable_route(name) {
                return Err(RegistrationError::DuplicateTool {
                    name: name.to_string(),
                });
            }
        }
        for uri in other.resources.keys() {
            if self.resources.contains_key(uri) {
                return Err(RegistrationError::DuplicateResource { uri: uri.clone() });
            }
        }
        self.tools.merge(other.tools);
        self.resources.extend(other.resources);
        Ok(())
    }

    /// Verify every existing tool UI association without mutating registration.
    ///
    /// Checks disabled routes too, so re-enabling an imported route cannot reveal
    /// an unchecked association. Both nested `ui.resourceUri` and deprecated
    /// `ui/resourceUri` declarations must be strings, valid UI URIs, agree when
    /// both exist and refer to a registered resource. Unknown metadata is retained.
    /// Ordinary tools without an association are valid. Imported rmcp map keys
    /// must agree with their declarations so listing, lookup and dispatch use
    /// the same tool identity.
    ///
    /// # Errors
    ///
    /// Returns URI, metadata, malformed-association, tool-name mismatch or
    /// missing-resource errors.
    /// It does not establish HTML validity, authorization or host compatibility.
    pub fn validate(&self) -> Result<(), RegistrationError> {
        self.validate_tool_names()?;
        for route in self.tools.map.values() {
            self.validate_tool(&route.attr)?;
        }
        Ok(())
    }

    /// Return an enabled tool's declaration, including its composed UI metadata.
    ///
    /// Disabled and unknown tools return `None`, matching rmcp routing behavior.
    /// Custom handlers should use this for `ServerHandler::get_tool` as well as
    /// this router's list and call operations.
    pub fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.get(name).cloned()
    }

    /// List all enabled tools, sorted by name, without pagination.
    ///
    /// The result has no next cursor and advertises zero cache lifetime with
    /// private cache scope. A custom handler owns any per-user filtering.
    ///
    /// # Errors
    ///
    /// Returns an invalid-parameters error for any supplied cursor; this router
    /// cannot interpret another handler's pagination state.
    pub fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
    ) -> Result<ListToolsResult, ErrorData> {
        reject_cursor(request)?;
        Ok(ListToolsResult::with_all_items(self.tools.list_all())
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }

    /// List registered resources sorted by exact URI, without invoking callbacks.
    ///
    /// The result has no next cursor and advertises zero cache lifetime with
    /// private cache scope. Resource security metadata is returned on HTML content.
    ///
    /// # Errors
    ///
    /// Returns an invalid-parameters error for any supplied cursor. A custom
    /// handler must define its own pagination when combining other resource sets.
    pub fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult, ErrorData> {
        reject_cursor(request)?;
        Ok(ListResourcesResult::with_all_items(
            self.resources
                .values()
                .map(|route| route.resource.descriptor())
                .collect(),
        )
        .with_ttl_ms(0)
        .with_cache_scope(CacheScope::Private))
    }

    /// Invoke rmcp's existing tool route with the original request and context.
    ///
    /// Parameter extraction, tool execution, MRTR input responses and errors retain
    /// rmcp's behavior. The state is borrowed for the call. The consuming handler
    /// remains responsible for any authorization before dispatch or within a tool.
    ///
    /// # Errors
    ///
    /// Propagates rmcp routing or handler errors without inventing fallback routes.
    pub async fn call_tool(
        &self,
        state: &S,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.tools
            .call(ToolCallContext::new(state, request, context))
            .await
    }

    /// Read matching static HTML or invoke its dynamic callback exactly once.
    ///
    /// Successful content receives the registered URI, MCP Apps MIME type and
    /// immutable prepared metadata. The callback receives the original context.
    /// The state is borrowed; no runtime or background task is created. Complete
    /// results have zero cache lifetime and private cache scope.
    ///
    /// # Errors
    ///
    /// Returns a resource-not-found error before invoking any callback for an
    /// unknown URI. Callback errors pass through unchanged. rmcp may apply its
    /// normal version-dependent error mapping when dispatching protocol requests.
    pub async fn read_resource(
        &self,
        state: &S,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let route = self
            .resources
            .get(&request.uri)
            .ok_or_else(|| ErrorData::resource_not_found("UI resource is not registered", None))?;
        let html = match &route.source {
            HtmlSource::Static(html) => html.clone(),
            HtmlSource::Callback(callback) => callback(state, context).await?,
        };
        let content = route
            .resource
            .html_contents(html)
            .with_meta(route.meta.clone());
        Ok(ReadResourceResult::new(vec![content])
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private)
            .into())
    }

    /// Check exact resource identity before a custom handler chooses dispatch.
    ///
    /// No normalization, resource read or authorization occurs. A positive result
    /// means registration exists, not that the caller is permitted to read it.
    pub fn contains_resource(&self, uri: &str) -> bool {
        self.resources.contains_key(uri)
    }

    /// Advertise capabilities implemented by registered routes, preserving settings.
    ///
    /// Enables tools and resources only when their registration sets are nonempty,
    /// and the UI extension when resources exist. Existing capabilities, extension
    /// settings and notifications flags are left intact. This declares support;
    /// it does not implement callbacks for other capabilities supplied by a server.
    pub fn declare_capabilities(&self, capabilities: &mut ServerCapabilities) {
        if !self.tools.map.is_empty() {
            capabilities.tools.get_or_insert_default();
        }
        if !self.resources.is_empty() {
            capabilities.resources.get_or_insert_default();
            declare_ui_extension(capabilities);
        }
    }

    fn validate_tool_names(&self) -> Result<(), RegistrationError> {
        for (name, route) in &self.tools.map {
            if name != &route.attr.name {
                return Err(RegistrationError::ToolNameMismatch {
                    registered_name: name.to_string(),
                    declared_name: route.attr.name.to_string(),
                });
            }
        }
        Ok(())
    }

    fn validate_tool(&self, tool: &Tool) -> Result<(), RegistrationError> {
        let Some(meta) = &tool.meta else {
            return Ok(());
        };
        let mut candidate = meta.clone();
        ToolUi::default().merge_into(&mut candidate)?;
        let nested = meta
            .0
            .get("ui")
            .and_then(|value| value.as_object())
            .and_then(|ui| ui.get("resourceUri"));
        let legacy = meta.0.get("ui/resourceUri");
        let mut association = None;
        for (value, path) in [
            (nested, "_meta.ui.resourceUri"),
            (legacy, "_meta.ui/resourceUri"),
        ] {
            if let Some(value) = value {
                let raw =
                    value
                        .as_str()
                        .ok_or_else(|| RegistrationError::InvalidToolAssociation {
                            tool: tool.name.to_string(),
                            path: path.into(),
                        })?;
                let uri = UiResourceUri::new(raw)?;
                // Reuse the foundation's alias agreement and atomic composition
                // policy instead of introducing a second conflict rule.
                ToolUi::new(uri.clone()).merge_into(&mut candidate)?;
                association = Some(uri);
            }
        }
        if let Some(uri) = association
            && !self.contains_resource(uri.as_str())
        {
            return Err(RegistrationError::UnregisteredResource {
                uri: uri.to_string(),
            });
        }
        Ok(())
    }
}

fn reject_cursor(request: Option<PaginatedRequestParams>) -> Result<(), ErrorData> {
    if request.is_some_and(|request| request.cursor.is_some()) {
        return Err(ErrorData::invalid_params(
            "AppRouter lists all entries and does not accept a pagination cursor",
            None,
        ));
    }
    Ok(())
}
