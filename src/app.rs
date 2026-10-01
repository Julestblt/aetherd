use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::watch;

use crate::config::{ProvidersConfig, SamplingConfig, TailscaleConfig, TailscaleUnavailable};
use crate::providers::ProviderCache;
use crate::sampling::{Section, Shutdown, SnapshotBuilder, SystemSnapshot};
use crate::system::{MountStats, RealMountStats, SystemPaths};
use crate::tailscale::{TailscaleCache, TailscaleSnapshot, initial_section};

mod router;

pub use router::build_router;

/// Shared state handed to every request handler.
#[derive(Clone, Debug)]
pub struct AppState {
    pub(crate) started_at: OffsetDateTime,
    pub(crate) paths: SystemPaths,
    pub(crate) mount_stats: Arc<dyn MountStats>,
    pub(crate) sampling: SamplingConfig,
    tailscale: TailscaleConfig,
    tailscale_cache: TailscaleCache,
    providers: ProvidersConfig,
    provider_cache: ProviderCache,
    snapshots: watch::Sender<Option<Arc<SystemSnapshot>>>,
    shutdown: watch::Sender<bool>,
}

impl AppState {
    /// Creates application state with real filesystem statistics and default
    /// sampling settings.
    #[must_use]
    pub fn new(paths: SystemPaths) -> Self {
        Self::with_sampling(paths, SamplingConfig::default())
    }

    /// Creates application state with real filesystem statistics and the given
    /// sampling settings.
    #[must_use]
    pub fn with_sampling(paths: SystemPaths, sampling: SamplingConfig) -> Self {
        Self::with_mount_stats_and_sampling(paths, Arc::new(RealMountStats), sampling)
    }

    /// Creates application state with an injected filesystem-statistics source
    /// and default sampling settings.
    #[must_use]
    pub fn with_mount_stats(paths: SystemPaths, mount_stats: Arc<dyn MountStats>) -> Self {
        Self::with_mount_stats_and_sampling(paths, mount_stats, SamplingConfig::default())
    }

    /// Creates application state with injected disk statistics and sampling
    /// settings.
    #[must_use]
    pub fn with_mount_stats_and_sampling(
        paths: SystemPaths,
        mount_stats: Arc<dyn MountStats>,
        sampling: SamplingConfig,
    ) -> Self {
        let (snapshots, _) = watch::channel(None);
        let (shutdown, _) = watch::channel(false);
        let provider_cache = ProviderCache::new();
        provider_cache.configure("codex", "Codex", false, None);
        provider_cache.configure("opencode-go", "OpenCode Go", false, None);
        Self {
            started_at: OffsetDateTime::now_utc(),
            paths,
            mount_stats,
            sampling,
            tailscale: TailscaleConfig::default(),
            tailscale_cache: TailscaleCache::unavailable(TailscaleUnavailable::Disabled.reason()),
            providers: ProvidersConfig::default(),
            provider_cache,
            snapshots,
            shutdown,
        }
    }

    /// Configures optional Tailscale telemetry.
    ///
    /// A disabled or incomplete configuration is accepted: Tailscale is
    /// reported as unavailable instead of preventing startup.
    #[must_use]
    pub fn with_tailscale(mut self, tailscale: TailscaleConfig) -> Self {
        self.tailscale_cache = TailscaleCache::new(initial_section(&tailscale));
        self.tailscale = tailscale;
        self
    }

    /// Configures independent AI usage providers.
    #[must_use]
    pub fn with_providers(mut self, providers: ProvidersConfig) -> Self {
        let cache = ProviderCache::new();
        cache.configure("codex", "Codex", providers.codex.enabled, None);
        cache.configure(
            "opencode-go",
            "OpenCode Go",
            providers.opencode.enabled,
            None,
        );
        self.providers = providers;
        self.provider_cache = cache;
        self
    }

    /// Publishes a Tailscale snapshot as the latest available value.
    pub fn publish_tailscale(&self, snapshot: TailscaleSnapshot) {
        self.tailscale_cache
            .publish(Section::Available { value: snapshot });
    }

    /// Creates a snapshot builder wired to this state's telemetry sources.
    #[must_use]
    pub fn snapshot_builder(&self) -> SnapshotBuilder {
        SnapshotBuilder::new(self.paths.clone(), Arc::clone(&self.mount_stats))
            .with_tailscale(self.tailscale_cache.subscribe())
    }

    /// Publishes a snapshot as the latest sample, replacing any previous one.
    pub fn publish(&self, snapshot: SystemSnapshot) {
        self.snapshots.send_replace(Some(Arc::new(snapshot)));
    }

    /// Requests cooperative shutdown of the sampler and streaming handlers.
    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    pub(crate) fn snapshot_sender(&self) -> watch::Sender<Option<Arc<SystemSnapshot>>> {
        self.snapshots.clone()
    }

    pub(crate) fn snapshot(&self) -> watch::Ref<'_, Option<Arc<SystemSnapshot>>> {
        self.snapshots.borrow()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<Option<Arc<SystemSnapshot>>> {
        self.snapshots.subscribe()
    }

    pub(crate) fn shutdown_receiver(&self) -> Shutdown {
        Shutdown::receiver(&self.shutdown)
    }

    pub(crate) fn tailscale_config(&self) -> &TailscaleConfig {
        &self.tailscale
    }

    pub(crate) fn tailscale_cache(&self) -> &TailscaleCache {
        &self.tailscale_cache
    }

    pub(crate) fn providers_config(&self) -> &ProvidersConfig {
        &self.providers
    }

    pub(crate) fn provider_cache(&self) -> &ProviderCache {
        &self.provider_cache
    }
}
