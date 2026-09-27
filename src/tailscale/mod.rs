//! Tailscale tailnet telemetry from the official HTTP API.

mod cache;
mod client;
mod model;
mod refresh;

pub(crate) use cache::TailscaleCache;
pub use model::{TailscaleDevice, TailscaleSnapshot};
pub(crate) use refresh::initial_section;
pub use refresh::spawn_tailscale_refresher;
