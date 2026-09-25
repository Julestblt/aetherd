//! Integration tests for the background sampling task.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;

#[tokio::test(start_paused = true)]
async fn sampler_publishes_snapshots_and_stops_on_shutdown() {
    let mount_stats = Arc::new(common::FakeMountStats::default());
    let state = common::unpublished_state(common::fixture_paths(), mount_stats);
    let router = aetherd::build_router(state.clone());

    let sampler = aetherd::spawn_sampler(&state);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let (status, body) = common::get_for(router, "/v1/system").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cpu"]["status"], "available");

    state.shutdown();
    sampler.await.expect("sampler task joins cleanly");
}
