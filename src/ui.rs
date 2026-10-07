//! Construction-only declarations for MCP Apps tool and resource metadata.
//!
//! These types serialize declarations under `_meta.ui`; they do not deserialize
//! existing metadata or preserve unknown fields through a round trip. Omitted
//! fields are absent declarations, not instructions to reset existing metadata.
//! The consuming server supplies application services and access controls, while
//! the host enforces resource security and tool visibility.

use serde::Serialize;

use crate::uri::UiResourceUri;

/// UI association and visibility declarations for a server tool.
///
/// Resource security settings belong in [`ResourceUi`], not this declaration.
///
/// ```
/// use mcp_apps_server::{ToolUi, UiResourceUri, UiVisibility};
///
/// let mut ui = ToolUi::new(UiResourceUri::new("ui://cart/view.html")?);
/// ui.visibility = Some(vec![UiVisibility::App]);
/// assert_eq!(serde_json::to_value(ui)?, serde_json::json!({
///     "resourceUri": "ui://cart/view.html",
///     "visibility": ["app"]
/// }));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ToolUi {
    /// Resource to render for this tool, when a UI association is declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_uri: Option<UiResourceUri>,
    /// Declared access scopes; omission uses the protocol default of model and app.
    ///
    /// An explicit empty vector is retained as an empty array, not replaced by
    /// the omitted-field default. The host applies these visibility declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Vec<UiVisibility>>,
    #[serde(skip)]
    pub(crate) legacy_uri: bool,
}

impl ToolUi {
    /// Associates a tool with a resource, leaving visibility undeclared.
    pub fn new(resource_uri: UiResourceUri) -> Self {
        Self {
            resource_uri: Some(resource_uri),
            visibility: None,
            legacy_uri: false,
        }
    }

    /// Enables the deprecated flat `ui/resourceUri` key in prepared metadata.
    ///
    /// Metadata preparation operations additionally emit that compatibility key
    /// when a resource URI is present. Serializing this declaration always emits
    /// only the fields of the nested `ui` object; the compatibility setting is
    /// not serialized into that object.
    pub fn with_legacy_uri(mut self) -> Self {
        self.legacy_uri = true;
        self
    }
}

/// An actor allowed to discover or invoke a tool under its UI visibility policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UiVisibility {
    /// The tool is visible to and callable by the model.
    Model,
    /// The tool is callable by an app from the same server connection.
    App,
}

/// Security and rendering declarations for a UI resource.
///
/// Use this declaration on a resource content item's `_meta.ui`, or as a static
/// listing declaration. These values request host behavior; they neither grant
/// browser permissions nor validate CSP entries, domain formats or HTML content.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ResourceUi {
    /// External origins needed by the interface, for host CSP enforcement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csp: Option<UiCsp>,
    /// Browser capabilities requested by the interface; the host may decline them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<UiPermissions>,
    /// Dedicated sandbox origin in the format expected by the target host.
    ///
    /// Format and validation rules depend on the host; the value is retained as
    /// supplied and is not interpreted as a universal URL or hostname format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Requests a visible host border and background when true, their absence
    /// when false; omission lets the host choose its default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefers_border: Option<bool>,
}

/// Origin declarations from which the host constructs a Content Security Policy.
///
/// Entries retain their supplied spelling without syntax validation. The host
/// enforces the resulting policy and may impose further restrictions. An explicit
/// empty vector remains an empty array in the serialized declaration.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UiCsp {
    /// Origins for fetch, XHR and WebSocket connections (`connect-src`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_domains: Option<Vec<String>>,
    /// Origins for scripts, styles, images, fonts and media.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_domains: Option<Vec<String>>,
    /// Origins for nested iframes (`frame-src`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_domains: Option<Vec<String>>,
    /// Allowed document base origins (`base-uri`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_uri_domains: Option<Vec<String>>,
}

/// Browser permission requests declared by a UI resource.
///
/// A present field serializes as an object-valued request (`{}`), not a boolean.
/// A request does not guarantee that the host or user grants the permission.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UiPermissions {
    /// Requests access to the camera.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<PermissionRequest>,
    /// Requests access to the microphone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microphone: Option<PermissionRequest>,
    /// Requests access to geolocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<PermissionRequest>,
    /// Requests clipboard write access (the `clipboard-write` browser feature).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard_write: Option<PermissionRequest>,
}

/// An object-valued request for a browser permission, serialized as `{}`.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct PermissionRequest {}

impl PermissionRequest {
    /// Constructs an empty permission request.
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resource_declarations_use_protocol_keys_and_permission_objects()
    -> Result<(), serde_json::Error> {
        // Independent wire names and object-valued permissions come from the
        // MCP Apps 2026-01-26 UI Resource Format, not Rust field spellings.
        let resource = ResourceUi {
            csp: Some(UiCsp {
                connect_domains: Some(vec!["wss://api.example.com".into()]),
                resource_domains: Some(vec!["https://*.example.com".into()]),
                frame_domains: Some(vec!["https://frame.example.com".into()]),
                base_uri_domains: Some(vec!["https://base.example.com".into()]),
            }),
            permissions: Some(UiPermissions {
                camera: Some(PermissionRequest::new()),
                microphone: Some(PermissionRequest::new()),
                geolocation: Some(PermissionRequest::new()),
                clipboard_write: Some(PermissionRequest::new()),
            }),
            domain: Some("host-defined-sandbox".into()),
            prefers_border: Some(false),
        };
        assert_eq!(
            serde_json::to_value(resource)?,
            json!({
                "csp": {
                    "connectDomains": ["wss://api.example.com"],
                    "resourceDomains": ["https://*.example.com"],
                    "frameDomains": ["https://frame.example.com"],
                    "baseUriDomains": ["https://base.example.com"]
                },
                "permissions": {"camera": {}, "microphone": {}, "geolocation": {}, "clipboardWrite": {}},
                "domain": "host-defined-sandbox",
                "prefersBorder": false
            })
        );
        Ok(())
    }

    #[test]
    fn omissions_remain_distinct_from_explicit_empty_declarations()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(serde_json::to_value(ToolUi::default())?, json!({}));
        assert_eq!(serde_json::to_value(ResourceUi::default())?, json!({}));
        let tool = ToolUi {
            resource_uri: Some(UiResourceUri::new("ui://cart/view")?),
            visibility: Some(vec![]),
            ..ToolUi::default()
        }
        .with_legacy_uri();
        assert_eq!(
            serde_json::to_value(tool)?,
            json!({
                "resourceUri": "ui://cart/view", "visibility": []
            })
        );
        let resource = ResourceUi {
            csp: Some(UiCsp {
                connect_domains: Some(vec![]),
                ..UiCsp::default()
            }),
            permissions: Some(UiPermissions::default()),
            ..ResourceUi::default()
        };
        assert_eq!(
            serde_json::to_value(resource)?,
            json!({
                "csp": {"connectDomains": []}, "permissions": {}
            })
        );
        Ok(())
    }
}
