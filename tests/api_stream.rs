//! Integration tests for the `GET /v1/system/stream` SSE endpoint.

#![allow(
    clippy::expect_used,
    reason = "the event-framing helper fails loudly when the stream is malformed"
)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use time::Duration as TimeDuration;
use tokio::time::timeout;

use aetherd::{MountStats, SnapshotBuilder};

fn data_of(event: &str) -> serde_json::Value {
    let data = event
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .expect("event has a data line");
    serde_json::from_str(data).expect("event data is valid JSON")
}

#[tokio::test]
async fn stream_responds_as_event_stream_with_a_first_snapshot() {
    let state = common::state_with(common::fixture_paths());
    let router = aetherd::build_router(state);
    let (status, content_type, mut body) = common::open_stream(router, "/v1/system/stream").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("text/event-stream")),
        "unexpected content type: {content_type:?}"
    );

    let event = timeout(Duration::from_secs(1), common::next_event(&mut body))
        .await
        .expect("first event arrives")
        .expect("stream is not empty");
    assert!(event.contains("event: system"));
    assert_eq!(data_of(&event)["cpu"]["status"], "available");
}

#[tokio::test]
async fn stream_emits_each_new_snapshot() {
    let paths = common::fixture_paths();
    let mount_stats: Arc<dyn MountStats> = Arc::new(common::FakeMountStats::default());
    let state = common::state_with_mount_stats(paths.clone(), Arc::clone(&mount_stats));
    let router = aetherd::build_router(state.clone());

    let (_, _, mut body) = common::open_stream(router, "/v1/system/stream").await;
    let first = timeout(Duration::from_secs(1), common::next_event(&mut body))
        .await
        .expect("first event arrives")
        .expect("stream is not empty");
    let first = data_of(&first);

    let updated = SnapshotBuilder::new(paths, mount_stats)
        .collect(common::sample_time() + TimeDuration::seconds(1));
    state.publish(updated);

    let second = timeout(Duration::from_secs(1), common::next_event(&mut body))
        .await
        .expect("second event arrives")
        .expect("stream is not empty");
    let second = data_of(&second);

    assert_ne!(first["collected_at"], second["collected_at"]);
}

#[tokio::test]
async fn disconnecting_a_client_does_not_affect_the_latest_snapshot() {
    let paths = common::fixture_paths();
    let mount_stats: Arc<dyn MountStats> = Arc::new(common::FakeMountStats::default());
    let state = common::state_with_mount_stats(paths.clone(), Arc::clone(&mount_stats));
    let router = aetherd::build_router(state.clone());

    let (_, _, mut body) = common::open_stream(router.clone(), "/v1/system/stream").await;
    let _ = timeout(Duration::from_secs(1), common::next_event(&mut body)).await;
    drop(body);

    let updated = SnapshotBuilder::new(paths, mount_stats)
        .collect(common::sample_time() + TimeDuration::seconds(5));
    state.publish(updated);

    let (_, overview) = common::get_for(router.clone(), "/v1/system").await;
    let expected = overview["collected_at"].clone();

    let (_, _, mut body) = common::open_stream(router, "/v1/system/stream").await;
    let event = timeout(Duration::from_secs(1), common::next_event(&mut body))
        .await
        .expect("new client receives the latest snapshot")
        .expect("stream is not empty");

    assert_eq!(data_of(&event)["collected_at"], expected);
}

#[tokio::test]
async fn shutdown_ends_open_streams() {
    let state = common::state_with(common::fixture_paths());
    let router = aetherd::build_router(state.clone());

    let (_, _, mut body) = common::open_stream(router, "/v1/system/stream").await;
    let _ = timeout(Duration::from_secs(1), common::next_event(&mut body)).await;

    state.shutdown();

    let ended = timeout(Duration::from_secs(1), common::next_event(&mut body))
        .await
        .expect("stream ends promptly after shutdown");
    assert!(ended.is_none());
}

#[tokio::test]
async fn stream_marks_unavailable_sections() {
    let paths = aetherd::SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/");
    let state = common::state_with(paths);
    let router = aetherd::build_router(state);

    let (_, _, mut body) = common::open_stream(router, "/v1/system/stream").await;
    let event = timeout(Duration::from_secs(1), common::next_event(&mut body))
        .await
        .expect("first event arrives")
        .expect("stream is not empty");
    let data = data_of(&event);

    assert_eq!(data["cpu"]["status"], "unavailable");
    assert!(data["cpu"]["reason"].is_string());
}
