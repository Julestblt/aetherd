use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ConfigError;
use super::SecretString;

/// Configuration for optional AI usage integrations.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ProvidersConfig {
    /// Codex account quota integration.
    pub codex: CodexConfig,
    /// `OpenCode` Go account quota integration.
    pub opencode: OpenCodeConfig,
}

impl ProvidersConfig {
    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        for (provider, seconds) in [
            ("codex", self.codex.refresh_interval_seconds),
            ("opencode", self.opencode.refresh_interval_seconds),
        ] {
            if !(1..=86_400).contains(&seconds) {
                return Err(ConfigError::InvalidProviderRefreshInterval {
                    provider,
                    value: seconds,
                });
            }
        }
        Ok(())
    }
}

/// Codex quota settings. The file contains credentials and is read only by the provider.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CodexConfig {
    /// Enables the Codex quota reader.
    pub enabled: bool,
    /// Explicit path to a Codex auth.json file.
    pub auth_file: Option<PathBuf>,
    /// Time between Codex quota refreshes, in seconds.
    pub refresh_interval_seconds: u64,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auth_file: None,
            refresh_interval_seconds: 60,
        }
    }
}

impl CodexConfig {
    /// Returns the refresh interval.
    #[must_use]
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(self.refresh_interval_seconds)
    }
}

/// `OpenCode` Go usage settings.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct OpenCodeConfig {
    /// Enables the `OpenCode` Go usage reader.
    pub enabled: bool,
    /// Service account API key for the Console usage API.
    #[serde(skip_serializing)]
    pub api_key: SecretString,
    /// Time between `OpenCode` Go quota refreshes, in seconds.
    pub refresh_interval_seconds: u64,
}

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: SecretString::default(),
            refresh_interval_seconds: 60,
        }
    }
}

impl OpenCodeConfig {
    /// Returns the refresh interval.
    #[must_use]
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(self.refresh_interval_seconds)
    }
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "figment::Jail fixes the error type of its test closures"
)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;

    use super::*;
    use crate::config::load;

    #[test]
    fn provider_environment_is_nested_and_validated() {
        Jail::expect_with(|jail| {
            jail.set_env("AETHERD_PROVIDERS__CODEX__ENABLED", "true");
            jail.set_env(
                "AETHERD_PROVIDERS__CODEX__AUTH_FILE",
                "/run/secrets/codex-auth.json",
            );
            jail.set_env("AETHERD_PROVIDERS__CODEX__REFRESH_INTERVAL_SECONDS", "30");
            jail.set_env("AETHERD_PROVIDERS__OPENCODE__ENABLED", "true");
            jail.set_env("AETHERD_PROVIDERS__OPENCODE__API_KEY", "oc_sk_test-secret");
            let config = load(None).expect("provider config");
            assert!(config.providers.codex.enabled);
            assert_eq!(
                config.providers.codex.auth_file,
                Some(PathBuf::from("/run/secrets/codex-auth.json"))
            );
            assert_eq!(
                config.providers.codex.refresh_interval(),
                Duration::from_secs(30)
            );
            assert!(config.providers.opencode.enabled);
            assert_eq!(
                config.providers.opencode.api_key.expose(),
                "oc_sk_test-secret"
            );
            assert!(!format!("{config:?}").contains("oc_sk_test-secret"));
            assert!(
                !serde_json::to_string(&config)
                    .expect("json")
                    .contains("oc_sk_test-secret")
            );
            assert!(
                serde_json::to_value(&config).expect("json")["providers"]["opencode"]
                    .get("api_key")
                    .is_none()
            );
            assert_eq!(
                config.providers.opencode.refresh_interval(),
                Duration::from_secs(60)
            );
            Ok(())
        });
    }

    #[test]
    fn zero_provider_interval_is_rejected() {
        Jail::expect_with(|jail| {
            jail.set_env("AETHERD_PROVIDERS__OPENCODE__REFRESH_INTERVAL_SECONDS", "0");
            assert!(matches!(
                load(None),
                Err(ConfigError::InvalidProviderRefreshInterval {
                    provider: "opencode",
                    value: 0
                })
            ));
            Ok(())
        });
    }

    #[test]
    fn removed_opencode_auth_file_is_rejected() {
        Jail::expect_with(|jail| {
            jail.set_env("AETHERD_PROVIDERS__OPENCODE__AUTH_FILE", "/tmp/auth.json");
            assert!(load(None).is_err());
            Ok(())
        });
    }
}
