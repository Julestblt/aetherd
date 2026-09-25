#![forbid(unsafe_code)]

//! aetherd exposes Linux system telemetry and, later, normalized AI provider
//! usage through a versioned HTTP API.

mod api;
mod app;
mod config;

pub use app::{AppState, build_router};
pub use config::{Config, ConfigError, HttpConfig, PathsConfig, load as load_config};
