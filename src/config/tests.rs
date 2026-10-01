#![allow(
    clippy::result_large_err,
    reason = "figment::Jail fixes the error type of its test closures"
)]

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
    assert!(!config.tailscale.enabled);
    assert_eq!(config.tailscale.refresh_interval_seconds, 60);
}

#[test]
fn tailscale_is_configurable_through_the_environment() {
    Jail::expect_with(|jail| {
        jail.set_env("AETHERD_TAILSCALE__ENABLED", "true");
        jail.set_env("AETHERD_TAILSCALE__TAILNET", "example.ts.net");
        jail.set_env("AETHERD_TAILSCALE__API_KEY", "tskey-secret");
        jail.set_env("AETHERD_TAILSCALE__REFRESH_INTERVAL_SECONDS", "30");
        let config = load(None).expect("tailscale environment loads");
        assert!(config.tailscale.enabled);
        assert_eq!(config.tailscale.tailnet, "example.ts.net");
        assert_eq!(config.tailscale.refresh_interval_seconds, 30);
        assert_eq!(config.tailscale.unavailable(), None);
        assert!(!format!("{config:?}").contains("tskey-secret"));
        Ok(())
    });
}

#[test]
fn invalid_tailscale_refresh_interval_is_rejected() {
    Jail::expect_with(|jail| {
        jail.set_env("AETHERD_TAILSCALE__REFRESH_INTERVAL_SECONDS", "0");
        let error = load(None).expect_err("zero refresh interval must fail");
        assert!(matches!(
            error,
            ConfigError::InvalidTailscaleRefreshInterval { .. }
        ));
        Ok(())
    });
}

#[test]
fn unknown_tailscale_keys_are_rejected() {
    Jail::expect_with(|jail| {
        jail.create_file("aetherd.toml", "[tailscale]\nunknown = true\n")?;
        assert!(matches!(load(None), Err(ConfigError::Invalid(_))));
        Ok(())
    });
}

#[test]
fn sampling_interval_converts_to_duration() {
    let sampling = SamplingConfig { interval_ms: 2500 };
    assert_eq!(sampling.interval(), Duration::from_millis(2500));
}

#[test]
fn sampling_interval_accepts_range_boundaries() {
    for interval_ms in [
        SamplingConfig::MIN_INTERVAL_MS,
        SamplingConfig::MAX_INTERVAL_MS,
    ] {
        assert!(SamplingConfig { interval_ms }.validate().is_ok());
    }
}

#[test]
fn sampling_interval_rejects_invalid_values() {
    for interval_ms in [
        0,
        SamplingConfig::MIN_INTERVAL_MS - 1,
        SamplingConfig::MAX_INTERVAL_MS + 1,
    ] {
        assert!(matches!(
            SamplingConfig { interval_ms }.validate(),
            Err(ConfigError::InvalidSamplingInterval { .. })
        ));
    }
}

#[test]
fn missing_default_file_is_optional() {
    Jail::expect_with(|_jail| {
        assert_eq!(
            load(None).expect("defaults").paths.proc,
            PathBuf::from("/proc")
        );
        Ok(())
    });
}

#[test]
fn explicit_missing_file_is_rejected() {
    assert!(matches!(
        load(Some(Path::new("/nonexistent/aetherd.toml"))),
        Err(ConfigError::NotFound { .. })
    ));
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
        assert_eq!(
            load(None).expect("valid interval").sampling.interval_ms,
            250
        );
        jail.set_env("AETHERD_SAMPLING__INTERVAL_MS", "5");
        assert!(matches!(
            load(None),
            Err(ConfigError::InvalidSamplingInterval { .. })
        ));
        Ok(())
    });
}

#[test]
fn unknown_keys_are_rejected() {
    Jail::expect_with(|jail| {
        jail.create_file("aetherd.toml", "[paths]\nunknown = \"/tmp\"\n")?;
        assert!(matches!(load(None), Err(ConfigError::Invalid(_))));
        Ok(())
    });
}
