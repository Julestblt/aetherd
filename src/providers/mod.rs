//! Independent AI usage providers and their normalized quota model.

mod auth;
mod cache;
mod codex;
mod http;
mod model;
mod opencode;
mod refresh;

#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod tests;

use std::fmt;
use std::future::Future;
use std::pin::Pin;

pub use cache::ProviderCache;
pub use model::{
    ModelUsage, ProviderState, ProviderStatus, ProviderUsage, UsageSnapshot, UsageWindow,
    WindowKind,
};
pub use refresh::spawn_provider_refreshers;

pub(crate) use codex::CodexProvider;
pub(crate) use opencode::OpenCodeGoProvider;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderError {
    #[error("{0}")]
    AuthFile(&'static str),
    #[error("provider request failed or timed out")]
    Request,
    #[error("provider returned HTTP {0}")]
    Status(reqwest::StatusCode),
    #[error("provider returned invalid usage data")]
    InvalidData,
}

pub(crate) trait UsageProvider: fmt::Debug + Send + Sync {
    fn id(&self) -> &'static str;
    fn collect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderUsage, ProviderError>> + Send + '_>>;
}
