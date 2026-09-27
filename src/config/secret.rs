use std::fmt;

use serde::{Deserialize, Serialize};

/// Placeholder that replaces a secret in serialized output.
const REDACTED: &str = "[redacted]";

/// A value that must never reach logs, traces, or API payloads.
///
/// [`fmt::Debug`] and `Serialize` output are both redacted, so the type can be
/// carried through configuration and client structs without leaking the secret
/// by accident.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps a secret value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Reports whether the secret is absent.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the secret for an outgoing authenticated request.
    #[must_use]
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString([redacted])")
    }
}

impl Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(REDACTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_is_redacted() {
        let secret = SecretString::new("tskey-secret-value");
        let rendered = format!("{secret:?}");

        assert!(!rendered.contains("tskey-secret-value"));
        assert_eq!(rendered, "SecretString([redacted])");
    }

    #[test]
    fn serialized_output_is_redacted() {
        let secret = SecretString::new("tskey-secret-value");
        let json = serde_json::to_string(&secret).expect("secret serializes");

        assert_eq!(json, "\"[redacted]\"");
    }

    #[test]
    fn deserializes_from_a_plain_string_and_reports_absence() {
        let secret: SecretString = serde_json::from_str("\"tskey-secret-value\"").expect("parses");

        assert_eq!(secret.expose(), "tskey-secret-value");
        assert!(!secret.is_empty());
        assert!(SecretString::default().is_empty());
    }
}
