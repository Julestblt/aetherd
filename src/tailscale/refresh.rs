use tokio::task::JoinHandle;

use super::cache::TailscaleCache;
use super::client::{DevicesClient, HttpTailscaleClient};
use super::model::TailscaleSnapshot;
use crate::app::AppState;
use crate::config::TailscaleConfig;
use crate::sampling::{Section, Shutdown};

/// Reason reported before the first Tailscale refresh completes.
pub(crate) const PENDING_REASON: &str = "tailscale data has not been collected yet";

/// Produces one Tailscale section per refresh from an injected client.
#[derive(Debug)]
pub(crate) struct TailscaleRefresh<C> {
    client: C,
    config: TailscaleConfig,
}

impl<C: DevicesClient> TailscaleRefresh<C> {
    pub(crate) fn new(client: C, config: TailscaleConfig) -> Self {
        Self { client, config }
    }

    /// Fetches the tailnet devices, mapping every failure to an unavailable
    /// section so a broken integration never fails the system snapshot.
    pub(crate) async fn refresh(&self) -> Section<TailscaleSnapshot> {
        if let Some(unavailable) = self.config.unavailable() {
            return Section::Unavailable {
                reason: unavailable.reason().to_owned(),
            };
        }

        match self.client.fetch(self.config.tailnet.trim()).await {
            Ok(snapshot) => Section::Available { value: snapshot },
            Err(error) => {
                tracing::warn!(error = %error, "tailscale refresh failed");
                Section::Unavailable {
                    reason: error.to_string(),
                }
            }
        }
    }

    fn interval(&self) -> std::time::Duration {
        self.config.refresh_interval()
    }
}

/// Section reported before the refresher has produced its first result.
pub(crate) fn initial_section(config: &TailscaleConfig) -> Section<TailscaleSnapshot> {
    let reason = config
        .unavailable()
        .map_or(PENDING_REASON, |reason| reason.reason());
    Section::Unavailable {
        reason: reason.to_owned(),
    }
}

/// Spawns the Tailscale refresh task and returns its handle.
///
/// Returns `None` when the integration is disabled or incomplete, so the daemon
/// starts without Tailscale telemetry. The task refreshes immediately, then on
/// the configured interval until shutdown. It never outlives the daemon.
pub fn spawn_tailscale_refresher(state: &AppState) -> Option<JoinHandle<()>> {
    let config = state.tailscale_config().clone();
    if config.unavailable().is_some() {
        return None;
    }

    let client = match HttpTailscaleClient::new(config.api_key.clone()) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "tailscale client unavailable");
            return None;
        }
    };

    Some(spawn(
        state.tailscale_cache().clone(),
        TailscaleRefresh::new(client, config),
        state.shutdown_receiver(),
    ))
}

/// Spawns the refresh loop over an injected client boundary.
pub(crate) fn spawn<C>(
    cache: TailscaleCache,
    refresh: TailscaleRefresh<C>,
    mut shutdown: Shutdown,
) -> JoinHandle<()>
where
    C: DevicesClient + 'static,
{
    tokio::spawn(async move {
        loop {
            cache.publish(refresh.refresh().await);

            tokio::select! {
                () = tokio::time::sleep(refresh.interval()) => {}
                () = shutdown.recv() => break,
            }
        }

        tracing::debug!("tailscale refresher stopped");
    })
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use time::OffsetDateTime;

    use super::*;
    use crate::config::SecretString;
    use crate::tailscale::client::TailscaleError;

    #[derive(Debug)]
    struct FixedClient {
        snapshot: Option<TailscaleSnapshot>,
    }

    impl DevicesClient for FixedClient {
        fn fetch(
            &self,
            _tailnet: &str,
        ) -> impl Future<Output = Result<TailscaleSnapshot, TailscaleError>> + Send {
            let result = self.snapshot.clone().ok_or(TailscaleError::Status(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            ));
            async move { result }
        }
    }

    fn snapshot() -> TailscaleSnapshot {
        TailscaleSnapshot {
            collected_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
            tailnet: "example.ts.net".to_owned(),
            devices: Vec::new(),
        }
    }

    fn enabled() -> TailscaleConfig {
        TailscaleConfig {
            enabled: true,
            tailnet: "example.ts.net".to_owned(),
            api_key: SecretString::new("tskey-secret"),
            refresh_interval_seconds: 60,
        }
    }

    fn refresh(snapshot: Option<TailscaleSnapshot>) -> TailscaleRefresh<FixedClient> {
        TailscaleRefresh::new(FixedClient { snapshot }, enabled())
    }

    #[test]
    fn initial_section_reports_the_configuration_reason() {
        assert_eq!(
            initial_section(&TailscaleConfig::default()),
            Section::Unavailable {
                reason: "tailscale integration is disabled".to_owned()
            }
        );
        assert_eq!(
            initial_section(&enabled()),
            Section::Unavailable {
                reason: PENDING_REASON.to_owned()
            }
        );
    }

    #[tokio::test]
    async fn refresh_maps_devices_to_an_available_section() {
        let section = refresh(Some(snapshot())).refresh().await;

        assert!(matches!(section, Section::Available { .. }));
    }

    #[tokio::test]
    async fn refresh_maps_failures_to_an_unavailable_section() {
        let section = refresh(None).refresh().await;

        match section {
            Section::Unavailable { reason } => {
                assert!(reason.contains("500"));
                assert!(!reason.contains("tskey-secret"));
            }
            Section::Available { .. } => panic!("failure must not be available"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn refresher_publishes_and_survives_a_failing_refresh() {
        let cache = TailscaleCache::unavailable(PENDING_REASON);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let task = spawn(cache.clone(), refresh(None), Shutdown::receiver(&shutdown));

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert!(matches!(
            &*cache.subscribe().borrow(),
            Section::Unavailable { .. }
        ));

        shutdown.send_replace(true);
        task.await.expect("refresher joins cleanly");
    }

    #[tokio::test(start_paused = true)]
    async fn refresher_stops_on_shutdown() {
        let cache = TailscaleCache::unavailable(PENDING_REASON);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let task = spawn(
            cache,
            refresh(Some(snapshot())),
            Shutdown::receiver(&shutdown),
        );

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        shutdown.send_replace(true);
        task.await.expect("refresher joins cleanly");
    }
}
