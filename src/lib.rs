#![forbid(unsafe_code)]

//! aetherd exposes Linux system telemetry and, later, normalized AI provider
//! usage through a versioned HTTP API.

mod api;
mod app;
mod config;
mod sampling;
mod system;
mod tailscale;

pub use app::{AppState, build_router};
pub use config::{
    Config, ConfigError, HttpConfig, PathsConfig, SamplingConfig, SecretString, TailscaleConfig,
    TailscaleUnavailable, load as load_config,
};
pub use sampling::{SnapshotBuilder, SystemSnapshot, spawn_sampler};
pub use system::{DiskUsage, MountStats, SystemPaths};
pub use tailscale::{TailscaleDevice, TailscaleSnapshot, spawn_tailscale_refresher};
