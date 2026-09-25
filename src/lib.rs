#![forbid(unsafe_code)]

//! aetherd exposes Linux system telemetry and, later, normalized AI provider
//! usage through a versioned HTTP API.

mod api;
mod app;
mod config;
mod sampling;
mod system;

pub use app::{AppState, build_router};
pub use config::{
    Config, ConfigError, HttpConfig, PathsConfig, SamplingConfig, load as load_config,
};
pub use sampling::{SnapshotBuilder, SystemSnapshot, spawn_sampler};
pub use system::{DiskUsage, MountStats, SystemPaths};
