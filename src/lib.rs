#![forbid(unsafe_code)]

//! aetherd exposes Linux system telemetry and, later, normalized AI provider
//! usage through a versioned HTTP API.

mod api;
mod app;

pub use app::{AppState, build_router};
