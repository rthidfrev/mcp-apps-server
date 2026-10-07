use rmcp::model::{Resource, ResourceContents};

use crate::{MetadataError, UI_MIME_TYPE, UiResourceUri};

/// A UI resource whose HTML is embedded in the consuming binary.
///
/// The application router calls these methods during registration. They describe
/// content and metadata; they do not load files, execute requests or authorize
/// access. For HTML computed per request, use a dynamic resource route instead.
/// Declarations must stay fixed and have no side effects: several tools can prepare
/// the same type, and shared URI registration compares their complete definitions.
/// The optional `macros` feature (also selected by `derive`) supplies a derive with
/// the same name. A fieldless type is sufficient; registration uses the type's
/// associated functions and never constructs an instance.
///
/// ```
/// use mcp_apps_server::{StaticUiResource, UiResource, UiResourceUri, UiUriError};
/// struct Dashboard;
/// impl StaticUiResource for Dashboard {
///     fn resource() -> Result<UiResource, UiUriError> {
///         Ok(UiResource::new(UiResourceUri::new("ui://inventory/dashboard")?, "dashboard"))
///     }
///     fn html() -> &'static str {
///         "<!doctype html><html><body>Inventory</body></html>"
///     }
/// }
/// assert_eq!(Dashboard::resource()?.uri().as_str(), "ui://inventory/dashboard");
/// # Ok::<(), UiUriError>(())
/// ```
pub trait StaticUiResource {
    /// Prepare the resource definition, validating its URI.
    ///
    /// Returns the URI error if a manually implemented declaration is invalid.
    fn resource() -> Result<UiResource, crate::UiUriError>;

    /// Return HTML5 content embedded in the consumer's binary.
    ///
    /// The library does not parse or sanitize this content. The derive uses
    /// `include_str!`, evaluated by the consumer's compiler, not by the server.
    fn html() -> &'static str;

    /// Prepare the complete open metadata map for returned HTML content.
    ///
    /// Defaults to an empty map. Registration preserves its unrelated keys and
    /// composes [`Self::ui`] into its `ui` object. These annotations appear on
    /// `resources/read` contents, not on the listing descriptor. Other extension
    /// SDKs own the types, validation and meaning of their annotations.
    ///
    /// Like the other declaration methods, this runs during registration, must
    /// return a fixed definition and must not perform I/O or have side effects.
    /// The optional derive accepts `meta = function_path` with this signature.
    ///
    /// # Errors
    ///
    /// Return preparation or serialization failures instead of panicking or
    /// omitting annotations. [`MetadataError::Preparation`] retains another
    /// SDK's error as a cause. Registration also rejects a non-object root `ui`
    /// or fields contradicting [`Self::ui`], without inserting the resource.
    ///
    /// ```
    /// use mcp_apps_server::{MetaObject, MetadataError};
    /// use serde::Serialize;
    /// #[derive(Serialize)]
    /// #[serde(rename_all = "camelCase")]
    /// struct DisplayPreferences { compact: bool }
    /// fn dashboard_meta() -> Result<MetaObject, MetadataError> {
    ///     let mut meta = MetaObject::default();
    ///     meta.0.insert("example/display".into(),
    ///         serde_json::to_value(DisplayPreferences { compact: true })?);
    ///     Ok(meta)
    /// }
    /// assert_eq!(dashboard_meta()?.0["example/display"]["compact"], true);
    /// # Ok::<(), MetadataError>(())
    /// ```
    fn meta() -> Result<crate::MetaObject, MetadataError> {
        Ok(crate::MetaObject::default())
    }

    /// Prepare typed resource metadata for returned content.
    ///
    /// Defaults to no UI settings. Security settings remain declarations for the
    /// host; they do not grant permissions or enforce access controls.
    fn ui() -> crate::ResourceUi {
        crate::ResourceUi::default()
    }
}

/// A named UI resource used to prepare a descriptor and matching HTML contents.
///
/// The URI is validated once and copied verbatim into both protocol objects.
/// HTML remains supplied by the consuming server, which owns loading, caching,
/// authorization and dispatch. This type does not register a resource.
#[derive(Debug, Clone)]
pub struct UiResource {
    uri: UiResourceUri,
    name: String,
    description: Option<String>,
}

impl UiResource {
    /// Define a UI resource from a validated URI and programmatic name.
    ///
    /// The name is passed through to rmcp. This operation does not validate names,
    /// check resource uniqueness, or inspect HTML.
    pub fn new(uri: UiResourceUri, name: impl Into<String>) -> Self {
        Self {
            uri,
            name: name.into(),
            description: None,
        }
    }

    /// Add a description to descriptors prepared from this definition.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Borrow the resource URI for a tool association or consumer dispatch.
    pub fn uri(&self) -> &UiResourceUri {
        &self.uri
    }

    /// Prepare a descriptor suitable for `resources/list`.
    ///
    /// The descriptor uses [`UI_MIME_TYPE`]. Consumers can customize other rmcp
    /// descriptor fields after preparation. Listing a UI-only resource is optional.
    pub fn descriptor(&self) -> Resource {
        let mut resource =
            Resource::new(self.uri.to_string(), &self.name).with_mime_type(UI_MIME_TYPE);
        resource.description.clone_from(&self.description);
        resource
    }

    /// Prepare caller-supplied HTML for a `resources/read` response.
    ///
    /// Sets the matching URI and exact [`UI_MIME_TYPE`], overriding rmcp's plain
    /// text default. The input is preserved without parsing or sanitization; the
    /// server must supply valid HTML5. Add [`crate::ResourceUi`] metadata to these
    /// contents for security/domain declarations. This operation has no file,
    /// network or host effects.
    ///
    /// ```
    /// use mcp_apps_server::{ResourceUi, UiResource, UiResourceUri};
    /// use rmcp::model::ReadResourceResult;
    /// let view = UiResource::new(UiResourceUri::new("ui://dashboard/view")?, "view");
    /// let mut ui = ResourceUi::default();
    /// ui.prefers_border = Some(true);
    /// let contents = view.html_contents("<!doctype html><html><body>View</body></html>")
    ///     .with_meta(ui.to_meta()?);
    /// let result = ReadResourceResult::new(vec![contents]);
    /// # assert_eq!(result.contents.len(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn html_contents(&self, html: impl Into<String>) -> ResourceContents {
        ResourceContents::text(html, self.uri.to_string()).with_mime_type(UI_MIME_TYPE)
    }
}
