use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use super::{CodexProvider, OpenCodeGoProvider, ProviderCache, UsageProvider};
use crate::app::AppState;
use crate::config::ProvidersConfig;
use crate::sampling::Shutdown;

#[derive(Debug, Default)]
struct ProviderRegistry {
    enabled: Vec<(Arc<dyn UsageProvider>, Duration)>,
}

impl ProviderRegistry {
    fn from_config(config: &ProvidersConfig, cache: &ProviderCache) -> Self {
        let mut registry = Self::default();
        if config.codex.enabled {
            registry.register(
                "codex",
                CodexProvider::new(&config.codex)
                    .map(|provider| Arc::new(provider) as Arc<dyn UsageProvider>),
                config.codex.refresh_interval(),
                cache,
            );
        }
        if config.opencode.enabled {
            registry.register(
                "opencode-go",
                OpenCodeGoProvider::new(&config.opencode)
                    .map(|provider| Arc::new(provider) as Arc<dyn UsageProvider>),
                config.opencode.refresh_interval(),
                cache,
            );
        }
        registry
    }

    fn register(
        &mut self,
        id: &str,
        provider: Result<Arc<dyn UsageProvider>, super::ProviderError>,
        interval: Duration,
        cache: &ProviderCache,
    ) {
        match provider {
            Ok(provider) => self.enabled.push((provider, interval)),
            Err(error) => cache.mark_unavailable(id, error.to_string()),
        }
    }
}

/// Starts one independent refresh task per enabled and configured provider.
#[must_use]
pub fn spawn_provider_refreshers(state: &AppState) -> Vec<JoinHandle<()>> {
    ProviderRegistry::from_config(state.providers_config(), state.provider_cache())
        .enabled
        .into_iter()
        .map(|(provider, interval)| {
            spawn(
                provider,
                state.provider_cache().clone(),
                interval,
                state.shutdown_receiver(),
            )
        })
        .collect()
}

pub(crate) fn spawn(
    provider: Arc<dyn UsageProvider>,
    cache: ProviderCache,
    interval: Duration,
    mut shutdown: Shutdown,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let result = provider.collect().await;
            cache.publish(provider.id(), result, time::OffsetDateTime::now_utc());
            tokio::select! {
                () = tokio::time::sleep(interval) => {}
                () = shutdown.recv() => break,
            }
        }
    })
}
