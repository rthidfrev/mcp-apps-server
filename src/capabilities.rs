//! Prepare and inspect standard MCP Apps extension capabilities.
//!
//! Consumers obtain client capabilities from their current request or supported
//! connection lifecycle. These helpers do not manage that lifecycle.

use std::{error::Error, fmt};

use rmcp::model::{ClientCapabilities, JsonObject, ServerCapabilities};
use serde_json::Value;

use crate::{UI_EXTENSION_ID, UI_MIME_TYPE};

/// A validated, borrowed client declaration of MCP Apps support.
///
/// Unknown settings remain in the original JSON object. This view neither
/// copies nor changes the client's declaration.
#[derive(Debug)]
pub struct UiCapability<'a> {
    raw: &'a JsonObject,
}

impl UiCapability<'_> {
    /// Iterate over the declared MIME types in their original order.
    ///
    /// The declaration has already been checked to contain only strings.
    /// Duplicate and unknown MIME types are retained.
    pub fn mime_types(&self) -> impl Iterator<Item = &str> {
        self.raw
            .get("mimeTypes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
    }

    /// Return whether the client declares the standard MCP Apps HTML MIME type.
    ///
    /// Matching uses the exact protocol value in [`UI_MIME_TYPE`].
    pub fn supports_html(&self) -> bool {
        self.mime_types().any(|mime_type| mime_type == UI_MIME_TYPE)
    }
}

/// A malformed MCP Apps client capability declaration.
#[derive(Debug)]
#[non_exhaustive]
pub enum CapabilityError {
    /// The declared UI extension has no required `mimeTypes` setting.
    MissingMimeTypes,
    /// The `mimeTypes` setting is not a JSON array.
    MimeTypesNotArray,
    /// An entry in `mimeTypes` is not a JSON string.
    MimeTypeNotString {
        /// Zero-based position of the malformed entry.
        index: usize,
    },
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMimeTypes => {
                formatter.write_str("MCP Apps client capability is missing mimeTypes")
            }
            Self::MimeTypesNotArray => {
                formatter.write_str("MCP Apps client capability mimeTypes must be an array")
            }
            Self::MimeTypeNotString { index } => write!(
                formatter,
                "MCP Apps client capability mimeTypes[{index}] must be a string"
            ),
        }
    }
}

impl Error for CapabilityError {}

/// Inspect the client's MCP Apps extension declaration.
///
/// Returns `Ok(None)` when the extension is absent. A valid declaration can
/// advertise no MIME types or types other than HTML; use
/// [`UiCapability::supports_html`] to check HTML support.
///
/// Unknown settings are retained without validation or mutation.
///
/// # Errors
///
/// Returns [`CapabilityError`] when the declared extension omits `mimeTypes`,
/// supplies a non-array value, or contains a non-string array member.
pub fn ui_capability(
    capabilities: &ClientCapabilities,
) -> Result<Option<UiCapability<'_>>, CapabilityError> {
    let Some(raw) = capabilities
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get(UI_EXTENSION_ID))
    else {
        return Ok(None);
    };

    let mime_types = raw
        .get("mimeTypes")
        .ok_or(CapabilityError::MissingMimeTypes)?
        .as_array()
        .ok_or(CapabilityError::MimeTypesNotArray)?;

    for (index, mime_type) in mime_types.iter().enumerate() {
        if !mime_type.is_string() {
            return Err(CapabilityError::MimeTypeNotString { index });
        }
    }

    Ok(Some(UiCapability { raw }))
}

/// Declare MCP Apps support in the server's extension capabilities.
///
/// Inserts an empty settings object only when the UI extension is absent.
/// Existing UI settings and unrelated extensions are preserved. This does not
/// enable the ordinary MCP tools or resources capabilities.
pub fn declare_ui_extension(capabilities: &mut ServerCapabilities) {
    capabilities
        .extensions
        .get_or_insert_default()
        .entry(UI_EXTENSION_ID.to_owned())
        .or_default();
}

#[cfg(test)]
mod tests {
    use rmcp::model::ExtensionCapabilities;
    use serde_json::json;

    use super::*;

    fn client_with_ui(settings: Value) -> ClientCapabilities {
        let mut capabilities = ClientCapabilities::default();
        let mut extensions = ExtensionCapabilities::new();
        extensions.insert(
            "example/other".into(),
            serde_json::from_value(json!({"x": 1})).unwrap(),
        );
        extensions.insert(
            UI_EXTENSION_ID.into(),
            serde_json::from_value(settings).unwrap(),
        );
        capabilities.extensions = Some(extensions);
        capabilities
    }

    #[test]
    fn client_capability_distinguishes_absence_invalid_shape_and_html_support() {
        assert!(
            ui_capability(&ClientCapabilities::default())
                .unwrap()
                .is_none()
        );
        let mut absent = client_with_ui(json!({"mimeTypes": []}));
        absent.extensions.as_mut().unwrap().remove(UI_EXTENSION_ID);
        assert!(ui_capability(&absent).unwrap().is_none());

        assert!(matches!(
            ui_capability(&client_with_ui(json!({}))),
            Err(CapabilityError::MissingMimeTypes)
        ));
        for value in [Value::Null, json!(UI_MIME_TYPE), json!({}), json!(true)] {
            assert!(matches!(
                ui_capability(&client_with_ui(json!({"mimeTypes": value}))),
                Err(CapabilityError::MimeTypesNotArray)
            ));
        }
        assert!(matches!(
            ui_capability(&client_with_ui(json!({"mimeTypes": [UI_MIME_TYPE, 1]}))),
            Err(CapabilityError::MimeTypeNotString { index: 1 })
        ));

        for (types, supports_html) in [
            (vec![], false),
            (
                vec!["text/html", "text/html; profile=mcp-app", "example/future"],
                false,
            ),
            (vec!["example/future", UI_MIME_TYPE, UI_MIME_TYPE], true),
        ] {
            let capabilities = client_with_ui(json!({
                "mimeTypes": types,
                "future": {"enabled": true}
            }));
            let original = capabilities.clone();
            let view = ui_capability(&capabilities).unwrap().unwrap();
            assert_eq!(view.mime_types().collect::<Vec<_>>(), types);
            assert_eq!(view.supports_html(), supports_html);
            assert_eq!(capabilities, original);
        }
    }

    #[test]
    fn server_declaration_is_additive_and_idempotent() {
        let mut capabilities = ServerCapabilities::default();
        declare_ui_extension(&mut capabilities);
        assert_eq!(
            capabilities.extensions.as_ref().unwrap()[UI_EXTENSION_ID],
            JsonObject::new()
        );
        assert!(capabilities.tools.is_none());
        assert!(capabilities.resources.is_none());
        let declared = capabilities.clone();
        declare_ui_extension(&mut capabilities);
        assert_eq!(capabilities, declared);

        let mut existing = ServerCapabilities::default();
        existing.extensions = client_with_ui(json!({"future": true})).extensions;
        let original = existing.clone();
        declare_ui_extension(&mut existing);
        declare_ui_extension(&mut existing);
        assert_eq!(existing, original);
    }
}
