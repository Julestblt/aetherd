use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

mod providers;
mod secret;
mod tailscale;

#[cfg(test)]
mod tests;

pub use providers::{CodexConfig, OpenCodeConfig, ProvidersConfig};
pub use secret::SecretString;
pub use tailscale::{TailscaleConfig, TailscaleUnavailable};

/// Name of the configuration file looked up in the working directory.
pub(crate) const DEFAULT_CONFIG_FILE: &str = "aetherd.toml";

/// Prefix for environment variables that override configuration.
pub(crate) const ENV_PREFIX: &str = "AETHERD_";

/// Fully resolved daemon configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// HTTP server settings.
    pub http: HttpConfig,
    /// Host filesystem roots used by telemetry collectors.
    pub paths: PathsConfig,
    /// Background sampling settings.
    pub sampling: SamplingConfig,
    /// Optional Tailscale tailnet telemetry.
    pub tailscale: TailscaleConfig,
    /// Optional AI usage providers.
    pub providers: ProvidersConfig,
}

/// HTTP server configuration.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct HttpConfig {
    /// Address the HTTP server binds to.
    pub bind: SocketAddr,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
        }
    }
}

/// Filesystem roots for telemetry collection.
///
/// Defaults target a process running directly on a host. Container deployments
/// point these at read-only host mounts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PathsConfig {
    /// Root of the procfs mount.
    pub proc: PathBuf,
    /// Root of the sysfs mount.
    pub sys: PathBuf,
    /// Root of the host filesystem.
    pub host_root: PathBuf,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            proc: PathBuf::from("/proc"),
            sys: PathBuf::from("/sys"),
            host_root: PathBuf::from("/"),
        }
    }
}

/// Background telemetry sampling configuration.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SamplingConfig {
    /// Interval between samples, in milliseconds.
    pub interval_ms: u64,
}

impl SamplingConfig {
    /// Smallest accepted interval, in milliseconds.
    pub const MIN_INTERVAL_MS: u64 = 100;
    /// Largest accepted interval, in milliseconds.
    pub const MAX_INTERVAL_MS: u64 = 3_600_000;

    /// Returns the sampling interval as a [`Duration`].
    #[must_use]
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }

    fn validate(self) -> Result<(), ConfigError> {
        if (Self::MIN_INTERVAL_MS..=Self::MAX_INTERVAL_MS).contains(&self.interval_ms) {
            Ok(())
        } else {
            Err(ConfigError::InvalidSamplingInterval {
                value: self.interval_ms,
                min: Self::MIN_INTERVAL_MS,
                max: Self::MAX_INTERVAL_MS,
            })
        }
    }
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self { interval_ms: 1000 }
    }
}

/// Failures that can occur while loading configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// An explicitly requested configuration file does not exist.
    #[error("configuration file not found: {}", .path.display())]
    NotFound {
        /// The requested path.
        path: PathBuf,
    },
    /// The layered configuration could not be extracted into [`Config`].
    #[error("invalid configuration")]
    Invalid(#[source] Box<figment::Error>),
    /// The sampling interval is outside the accepted range.
    #[error("sampling.interval_ms must be between {min} and {max} milliseconds, got {value}")]
    InvalidSamplingInterval {
        /// The rejected value.
        value: u64,
        /// Inclusive lower bound.
        min: u64,
        /// Inclusive upper bound.
        max: u64,
    },
    /// The Tailscale refresh interval is outside the accepted range.
    #[error(
        "tailscale.refresh_interval_seconds must be between {min} and {max} seconds, got {value}"
    )]
    InvalidTailscaleRefreshInterval {
        /// The rejected value.
        value: u64,
        /// Inclusive lower bound.
        min: u64,
        /// Inclusive upper bound.
        max: u64,
    },
    /// A provider refresh interval is outside the supported range.
    #[error(
        "providers.{provider}.refresh_interval_seconds must be between 1 and 86400 seconds, got {value}"
    )]
    InvalidProviderRefreshInterval {
        /// Configured provider.
        provider: &'static str,
        /// Rejected number of seconds.
        value: u64,
    },
}

/// Loads configuration from defaults, an optional TOML file, and environment
/// variables.
///
/// The file is optional: when `explicit_path` is `None` and the default file is
/// absent, defaults and `AETHERD_`-prefixed environment variables are enough.
/// An explicitly requested path that does not exist is an error.
pub fn load(explicit_path: Option<&Path>) -> Result<Config, ConfigError> {
    let mut figment = Figment::from(Serialized::defaults(Config::default()));

    let file = explicit_path.map(Path::to_path_buf).or_else(|| {
        let default = PathBuf::from(DEFAULT_CONFIG_FILE);
        default.exists().then_some(default)
    });

    if let Some(path) = file {
        if !path.exists() {
            return Err(ConfigError::NotFound { path });
        }
        figment = figment.merge(Toml::file(path));
    }

    figment
        .merge(Env::prefixed(ENV_PREFIX).split("__"))
        .extract()
        .map_err(|error| ConfigError::Invalid(Box::new(error)))
        .and_then(|config: Config| {
            config.sampling.validate()?;
            config.tailscale.validate()?;
            config.providers.validate()?;
            Ok(config)
        })
}
