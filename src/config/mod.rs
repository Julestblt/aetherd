use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

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
            Ok(config)
        })
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "figment::Jail fixes the large error type of its test closures"
)]
mod tests {
    use super::*;
    use figment::Jail;

    #[test]
    fn defaults_are_stable() {
        let config = Config::default();
        assert_eq!(config.http.bind, SocketAddr::from(([127, 0, 0, 1], 8080)));
        assert_eq!(config.paths.proc, PathBuf::from("/proc"));
        assert_eq!(config.paths.sys, PathBuf::from("/sys"));
        assert_eq!(config.paths.host_root, PathBuf::from("/"));
        assert_eq!(config.sampling.interval_ms, 1000);
    }

    #[test]
    fn sampling_interval_converts_to_duration() {
        let sampling = SamplingConfig { interval_ms: 2500 };
        assert_eq!(sampling.interval(), Duration::from_millis(2500));
    }

    #[test]
    fn sampling_interval_accepts_range_boundaries() {
        let min = SamplingConfig {
            interval_ms: SamplingConfig::MIN_INTERVAL_MS,
        };
        let max = SamplingConfig {
            interval_ms: SamplingConfig::MAX_INTERVAL_MS,
        };
        assert!(min.validate().is_ok());
        assert!(max.validate().is_ok());
    }

    #[test]
    fn sampling_interval_rejects_invalid_values() {
        let below = SamplingConfig {
            interval_ms: SamplingConfig::MIN_INTERVAL_MS - 1,
        };
        let above = SamplingConfig {
            interval_ms: SamplingConfig::MAX_INTERVAL_MS + 1,
        };
        let zero = SamplingConfig { interval_ms: 0 };

        for sampling in [below, above, zero] {
            let error = sampling.validate().expect_err("interval must be rejected");
            assert!(matches!(error, ConfigError::InvalidSamplingInterval { .. }));
        }
    }

    #[test]
    fn missing_default_file_is_optional() {
        Jail::expect_with(|_jail| {
            let config = load(None).expect("defaults load without a file");
            assert_eq!(config.paths.proc, PathBuf::from("/proc"));
            Ok(())
        });
    }

    #[test]
    fn explicit_missing_file_is_rejected() {
        let error = load(Some(Path::new("/nonexistent/aetherd.toml")))
            .expect_err("explicit missing file must fail");
        assert!(matches!(error, ConfigError::NotFound { .. }));
    }

    #[test]
    fn toml_file_overrides_defaults() {
        Jail::expect_with(|jail| {
            jail.create_file("custom.toml", "[paths]\nproc = \"/host/proc\"\n")?;
            let config = load(Some(Path::new("custom.toml"))).expect("file loads");
            assert_eq!(config.paths.proc, PathBuf::from("/host/proc"));
            assert_eq!(config.paths.sys, PathBuf::from("/sys"));
            Ok(())
        });
    }

    #[test]
    fn env_overrides_defaults_and_file() {
        Jail::expect_with(|jail| {
            jail.create_file("aetherd.toml", "[http]\nbind = \"127.0.0.1:7000\"\n")?;
            jail.set_env("AETHERD_HTTP__BIND", "0.0.0.0:9000");
            jail.set_env("AETHERD_PATHS__HOST_ROOT", "/host/root");
            let config = load(None).expect("env layer loads");
            assert_eq!(config.http.bind, SocketAddr::from(([0, 0, 0, 0], 9000)));
            assert_eq!(config.paths.host_root, PathBuf::from("/host/root"));
            Ok(())
        });
    }

    #[test]
    fn sampling_interval_is_configurable_and_validated() {
        Jail::expect_with(|jail| {
            jail.set_env("AETHERD_SAMPLING__INTERVAL_MS", "250");
            let config = load(None).expect("valid interval loads");
            assert_eq!(config.sampling.interval_ms, 250);

            jail.set_env("AETHERD_SAMPLING__INTERVAL_MS", "5");
            let error = load(None).expect_err("too small interval must fail");
            assert!(matches!(error, ConfigError::InvalidSamplingInterval { .. }));
            Ok(())
        });
    }

    #[test]
    fn unknown_keys_are_rejected() {
        Jail::expect_with(|jail| {
            jail.create_file("aetherd.toml", "[paths]\nunknown = \"/tmp\"\n")?;
            let error = load(None).expect_err("unknown keys must fail");
            assert!(matches!(error, ConfigError::Invalid(_)));
            Ok(())
        });
    }
}
