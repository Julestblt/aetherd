use std::sync::Arc;

use tokio::task::JoinHandle;

use super::builder::SnapshotBuilder;
use crate::app::AppState;

/// Spawns the background task that samples the system on a fixed interval.
///
/// The task publishes each snapshot into the shared watch channel and stops
/// when the application requests shutdown. It never outlives the daemon.
pub fn spawn_sampler(state: &AppState) -> JoinHandle<()> {
    let builder = SnapshotBuilder::new(state.paths.clone(), Arc::clone(&state.mount_stats));
    let snapshots = state.snapshot_sender();
    let interval = state.sampling.interval();
    let mut shutdown = state.shutdown_receiver();

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let snapshot = builder.collect(time::OffsetDateTime::now_utc());
                    snapshots.send_replace(Some(Arc::new(snapshot)));
                }
                () = shutdown.recv() => break,
            }
        }

        tracing::debug!("sampler stopped");
    })
}
