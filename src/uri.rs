//! Validated identifiers for MCP Apps UI resources.

use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An owned UI resource URI with valid RFC 3986 syntax and a literal `ui://` prefix.
///
/// The original spelling is retained. Equality, hashing and ordering compare that
/// spelling without normalization, so resource identifiers remain suitable as
/// exact lookup keys. No authority or nonempty path is required; even `ui://` is
/// accepted. Validation does not establish that the resource exists on a server.
///
/// ```
/// use mcp_apps_server::UiResourceUri;
///
/// let uri = UiResourceUri::new("ui://weather/dashboard.html")?;
/// assert_eq!(serde_json::to_string(&uri)?, "\"ui://weather/dashboard.html\"");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UiResourceUri(String);

impl UiResourceUri {
    /// Validates and retains a UI resource identifier.
    ///
    /// # Errors
    ///
    /// Returns [`UiUriError::WrongPrefix`] unless the value starts with the exact
    /// lowercase prefix `ui://`. Returns [`UiUriError::InvalidSyntax`] if the
    /// prefixed value is not a valid RFC 3986 URI.
    pub fn new(value: impl Into<String>) -> Result<Self, UiUriError> {
        let value = value.into();
        if !value.starts_with("ui://") {
            return Err(UiUriError::WrongPrefix);
        }
        fluent_uri::Uri::parse(value.as_str()).map_err(UiUriError::InvalidSyntax)?;
        Ok(Self(value))
    }

    /// Borrows the original, validated URI spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the original URI string, consuming this identifier.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for UiResourceUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for UiResourceUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for UiResourceUri {
    type Err = UiUriError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for UiResourceUri {
    type Error = UiUriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for UiResourceUri {
    type Error = UiUriError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for UiResourceUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for UiResourceUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// A failure to construct a [`UiResourceUri`].
#[derive(Debug)]
#[non_exhaustive]
pub enum UiUriError {
    /// The value does not start with the literal lowercase prefix `ui://`.
    WrongPrefix,
    /// The value has the required prefix but fails RFC 3986 URI parsing.
    InvalidSyntax(fluent_uri::ParseError),
}

impl fmt::Display for UiUriError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPrefix => formatter.write_str("UI resource URI must start with ui://"),
            Self::InvalidSyntax(cause) => {
                write!(formatter, "invalid UI resource URI syntax: {cause}")
            }
        }
    }
}

impl Error for UiUriError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::WrongPrefix => None,
            Self::InvalidSyntax(cause) => Some(cause),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialization_enforces_prefix_and_uri_syntax() {
        // The Apps content requirements require ui://; RFC 3986 forbids invalid
        // percent escapes, spaces and malformed IP literals in URI identifiers.
        for invalid in [
            "https://weather/view",
            "UI://weather/view",
            "ui://view/%ZZ",
            "ui://bad host/view",
            "ui://[broken]/view",
        ] {
            let result = serde_json::from_value::<UiResourceUri>(serde_json::json!(invalid));
            assert!(result.is_err(), "accepted invalid identifier: {invalid}");
        }
        let error = UiResourceUri::new("ui://view/%ZZ").expect_err("invalid percent escape");
        assert!(matches!(error, UiUriError::InvalidSyntax(_)));
        assert!(error.source().is_some());
    }

    #[test]
    fn serialization_retains_spelling_and_accepts_empty_authority() -> Result<(), Box<dyn Error>> {
        for valid in ["ui://", "ui://Weather/%7eview?mode=FULL#Top"] {
            let uri: UiResourceUri = serde_json::from_value(serde_json::json!(valid))?;
            assert_eq!(serde_json::to_value(uri)?, serde_json::json!(valid));
        }
        Ok(())
    }
}
