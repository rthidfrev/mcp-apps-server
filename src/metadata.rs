use std::{error::Error, fmt};

use rmcp::model::{JsonObject, MetaObject, Tool};
use serde_json::Value;

use crate::{ResourceUi, ToolUi};

/// Metadata could not be prepared or composed with existing annotations.
///
/// Failed composition leaves the destination unchanged, even when a conflict
/// occurs after other fields would otherwise have been inserted.
#[derive(Debug)]
#[non_exhaustive]
pub enum MetadataError {
    /// The existing root `ui` value is present but is not an object.
    InvalidUi,
    /// A provided field differs from an existing value.
    Conflict {
        /// The conflicting protocol path, for example `_meta.ui.resourceUri`.
        path: String,
    },
    /// Serialization of a typed declaration failed.
    Serialization(serde_json::Error),
    /// A static resource's metadata factory failed to prepare extension data.
    ///
    /// Wrap another SDK's error here to retain its cause without adding that
    /// SDK's types or validation rules to this library.
    Preparation(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUi => f.write_str("existing _meta.ui must be an object"),
            Self::Conflict { path } => write!(f, "metadata conflict at {path}"),
            Self::Serialization(error) => {
                write!(f, "cannot serialize metadata declaration: {error}")
            }
            Self::Preparation(error) => write!(f, "cannot prepare resource metadata: {error}"),
        }
    }
}

impl Error for MetadataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::Preparation(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for MetadataError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

impl ToolUi {
    /// Prepare complete metadata for an `rmcp` tool or tool macro.
    ///
    /// When `resource_uri` is supplied, the default emits only nested
    /// `ui.resourceUri`. Opt in to the deprecated flat alias with
    /// [`Self::with_legacy_uri`].
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError::Serialization`] if serialization fails.
    pub fn to_meta(&self) -> Result<MetaObject, MetadataError> {
        let mut meta = MetaObject::default();
        self.merge_into(&mut meta)?;
        Ok(meta)
    }

    /// Compose this declaration with existing tool metadata atomically.
    ///
    /// Unknown keys, including nested ones, survive. Omitted fields leave existing
    /// values intact. Equal values are accepted; different scalar or array values
    /// are conflicts. Objects merge recursively. An empty declaration does not
    /// create `ui`, but still rejects an existing non-object `ui` entry.
    ///
    /// When `resource_uri` is supplied, a present deprecated `ui/resourceUri`
    /// must equal it. The alias is preserved by default and emitted when
    /// [`Self::with_legacy_uri`] was selected. Resource existence is the consumer's
    /// responsibility.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError::InvalidUi`] for a non-object `ui`,
    /// [`MetadataError::Conflict`] for unequal provided fields or a mismatched
    /// legacy alias, or [`MetadataError::Serialization`] on serialization failure.
    pub fn merge_into(&self, meta: &mut MetaObject) -> Result<(), MetadataError> {
        *meta = self.merged_meta(meta)?;
        Ok(())
    }

    fn merged_meta(&self, meta: &MetaObject) -> Result<MetaObject, MetadataError> {
        let patch = serde_json::to_value(self)?;
        if let Some(uri) = &self.resource_uri
            && let Some(existing) = meta.0.get("ui/resourceUri")
            && existing.as_str() != Some(uri.as_str())
        {
            return Err(MetadataError::Conflict {
                path: "_meta.ui/resourceUri".into(),
            });
        }
        let mut candidate = merged_ui(meta, patch)?;
        if self.legacy_uri
            && let Some(uri) = &self.resource_uri
        {
            candidate
                .0
                .insert("ui/resourceUri".into(), Value::String(uri.to_string()));
        }
        Ok(candidate)
    }

    /// Associate an existing tool while preserving its other declarations.
    ///
    /// Only metadata changes. The name, schemas, annotations and handler remain
    /// owned by the consumer. An empty patch leaves absent metadata absent.
    /// The complete tool is unchanged on an error.
    ///
    /// # Errors
    ///
    /// Returns the same validation and conflict errors as [`Self::merge_into`].
    pub fn apply_to(&self, tool: &mut Tool) -> Result<(), MetadataError> {
        let meta = self.merged_meta(tool.meta.as_ref().unwrap_or(&MetaObject::default()))?;
        if tool.meta.is_some() || !meta.0.is_empty() {
            tool.meta = Some(meta);
        }
        Ok(())
    }
}

impl ResourceUi {
    /// Prepare resource-content metadata containing this `ui` declaration.
    ///
    /// Attach it to contents returned by `resources/read`. Security metadata on a
    /// tool is not a substitute. An empty declaration produces an empty map.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError::Serialization`] if serialization fails.
    pub fn to_meta(&self) -> Result<MetaObject, MetadataError> {
        let mut meta = MetaObject::default();
        self.merge_into(&mut meta)?;
        Ok(meta)
    }

    /// Compose resource UI metadata without discarding existing extension data.
    ///
    /// Omitted fields leave existing values intact. Objects merge recursively,
    /// including unknown CSP and permission fields; arrays and scalars must equal
    /// existing values. Explicit false, empty arrays and empty permission objects
    /// are declarations rather than omission. This does not validate host-specific
    /// domain formats or grant permissions.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError::InvalidUi`] for a non-object `ui`,
    /// [`MetadataError::Conflict`] for unequal provided fields, or
    /// [`MetadataError::Serialization`] on serialization failure. No mutation
    /// occurs on any error.
    pub fn merge_into(&self, meta: &mut MetaObject) -> Result<(), MetadataError> {
        *meta = merged_ui(meta, serde_json::to_value(self)?)?;
        Ok(())
    }
}

fn merged_ui(meta: &MetaObject, patch: Value) -> Result<MetaObject, MetadataError> {
    let patch: JsonObject = serde_json::from_value(patch)?;
    if meta.0.get("ui").is_some_and(|value| !value.is_object()) {
        return Err(MetadataError::InvalidUi);
    }
    let mut candidate = meta.clone();
    if !patch.is_empty() {
        match candidate.0.get_mut("ui") {
            Some(Value::Object(existing)) => merge_object(existing, patch, "_meta.ui")?,
            None => {
                candidate.0.insert("ui".into(), Value::Object(patch));
            }
            _ => return Err(MetadataError::InvalidUi),
        }
    }
    Ok(candidate)
}

fn merge_object(
    destination: &mut JsonObject,
    patch: JsonObject,
    path: &str,
) -> Result<(), MetadataError> {
    for (key, proposed) in patch {
        let field_path = format!("{path}.{key}");
        match destination.get_mut(&key) {
            Some(Value::Object(existing)) if proposed.is_object() => {
                if let Value::Object(fields) = proposed {
                    merge_object(existing, fields, &field_path)?;
                }
            }
            Some(existing) if *existing != proposed => {
                return Err(MetadataError::Conflict { path: field_path });
            }
            Some(_) => {}
            None => {
                destination.insert(key, proposed);
            }
        }
    }
    Ok(())
}
