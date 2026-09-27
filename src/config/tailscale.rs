use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ConfigError;
use super::secret::SecretString;

/// Optional Tailscale tailnet telemetry.
///
/// The integration polls the official Tailscale HTTP API; it never requires the
/// `tailscale` CLI or a host socket.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct TailscaleConfig {
    /// Whether tailnet machines are collected.
    pub enabled: bool,
    /// Tailnet to query, for example `example.ts.net`.
    pub tailnet: String,
    /// Tailscale API key. Never serialized, never logged.
    #[serde(skip_serializing)]
    pub api_key: SecretString,
    /// Interval between Tailscale API refreshes, in seconds.
    ///
    /// This is independent of the system sampling interval.
    pub refresh_interval_seconds: u64,
}

impl TailscaleConfig {
    /// Smallest accepted refresh interval, in seconds.
    pub const MIN_REFRESH_INTERVAL_SECONDS: u64 = 1;
    /// Largest accepted refresh interval, in seconds.
    pub const MAX_REFRESH_INTERVAL_SECONDS: u64 = 86_400;

    /// Returns the refresh interval as a [`Duration`].
    #[must_use]
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(self.refresh_interval_seconds)
    }

    /// Reports why the integration cannot run, or `None` when it is ready.
    ///
    /// A disabled or incomplete configuration is not an error: the daemon still
    /// starts and reports the Tailscale section as unavailable.
    #[must_use]
    pub fn unavailable(&self) -> Option<TailscaleUnavailable> {
        if !self.enabled {
            return Some(TailscaleUnavailable::Disabled);
        }
        if self.tailnet.trim().is_empty() {
            return Some(TailscaleUnavailable::MissingTailnet);
        }
        if self.api_key.is_empty() {
            return Some(TailscaleUnavailable::MissingApiKey);
        }
        None
    }

    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        if (Self::MIN_REFRESH_INTERVAL_SECONDS..=Self::MAX_REFRESH_INTERVAL_SECONDS)
            .contains(&self.refresh_interval_seconds)
        {
            Ok(())
        } else {
            Err(ConfigError::InvalidTailscaleRefreshInterval {
                value: self.refresh_interval_seconds,
                min: Self::MIN_REFRESH_INTERVAL_SECONDS,
                max: Self::MAX_REFRESH_INTERVAL_SECONDS,
            })
        }
    }
}

impl Default for TailscaleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tailnet: String::new(),
            api_key: SecretString::default(),
            refresh_interval_seconds: 60,
        }
    }
}

/// Why the Tailscale integration is not collecting data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailscaleUnavailable {
    /// The integration is switched off.
    Disabled,
    /// The integration is enabled without a tailnet.
    MissingTailnet,
    /// The integration is enabled without an API key.
    MissingApiKey,
}

impl TailscaleUnavailable {
    /// Secret-free explanation used in API responses and logs.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Disabled => "tailscale integration is disabled",
            Self::MissingTailnet => "tailscale integration is enabled but no tailnet is configured",
            Self::MissingApiKey => "tailscale integration is enabled but no API key is configured",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled() -> TailscaleConfig {
        TailscaleConfig {
            enabled: true,
            tailnet: "example.ts.net".to_owned(),
            api_key: SecretString::new("tskey-secret"),
            refresh_interval_seconds: 60,
        }
    }

    #[test]
    fn defaults_are_disabled_and_optional() {
        let config = TailscaleConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.refresh_interval_seconds, 60);
        assert_eq!(config.refresh_interval(), Duration::from_secs(60));
        assert_eq!(config.unavailable(), Some(TailscaleUnavailable::Disabled));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn a_complete_configuration_is_ready() {
        assert_eq!(enabled().unavailable(), None);
    }

    #[test]
    fn incomplete_configuration_reports_a_typed_reason() {
        let no_tailnet = TailscaleConfig {
            tailnet: "  ".to_owned(),
            ..enabled()
        };
        let no_key = TailscaleConfig {
            api_key: SecretString::default(),
            ..enabled()
        };

        assert_eq!(
            no_tailnet.unavailable(),
            Some(TailscaleUnavailable::MissingTailnet)
        );
        assert_eq!(
            no_key.unavailable(),
            Some(TailscaleUnavailable::MissingApiKey)
        );
    }

    #[test]
    fn unavailable_reasons_are_stable_and_secret_free() {
        let reasons = [
            TailscaleUnavailable::Disabled.reason(),
            TailscaleUnavailable::MissingTailnet.reason(),
            TailscaleUnavailable::MissingApiKey.reason(),
        ];

        for reason in reasons {
            assert!(!reason.is_empty());
            assert!(!reason.contains("tskey"));
        }
    }

    #[test]
    fn refresh_interval_accepts_range_boundaries() {
        let min = TailscaleConfig {
            refresh_interval_seconds: TailscaleConfig::MIN_REFRESH_INTERVAL_SECONDS,
            ..enabled()
        };
        let max = TailscaleConfig {
            refresh_interval_seconds: TailscaleConfig::MAX_REFRESH_INTERVAL_SECONDS,
            ..enabled()
        };

        assert!(min.validate().is_ok());
        assert!(max.validate().is_ok());
    }

    #[test]
    fn refresh_interval_rejects_zero_and_overflow() {
        let zero = TailscaleConfig {
            refresh_interval_seconds: 0,
            ..enabled()
        };
        let huge = TailscaleConfig {
            refresh_interval_seconds: TailscaleConfig::MAX_REFRESH_INTERVAL_SECONDS + 1,
            ..enabled()
        };

        for config in [zero, huge] {
            let error = config.validate().expect_err("interval must be rejected");
            assert!(matches!(
                error,
                ConfigError::InvalidTailscaleRefreshInterval { .. }
            ));
        }
    }

    #[test]
    fn debug_and_serialized_output_never_contain_the_api_key() {
        let config = enabled();

        assert!(!format!("{config:?}").contains("tskey-secret"));
        let json = serde_json::to_string(&config).expect("config serializes");
        assert!(!json.contains("tskey-secret"));
        assert!(!json.contains("api_key"));
    }
}
