use std::net::SocketAddr;
use std::path::{Path, PathBuf};

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
    fn unknown_keys_are_rejected() {
        Jail::expect_with(|jail| {
            jail.create_file("aetherd.toml", "[paths]\nunknown = \"/tmp\"\n")?;
            let error = load(None).expect_err("unknown keys must fail");
            assert!(matches!(error, ConfigError::Invalid(_)));
            Ok(())
        });
    }
}
