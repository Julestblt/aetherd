//! Integration tests for the `/v1/system` CPU and memory endpoints.

mod common;

use axum::http::StatusCode;

use aetherd::SystemPaths;

#[tokio::test]
async fn cpu_returns_metrics_from_fixture() {
    let (status, body) = common::get("/v1/system/cpu").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["usage_percent"].as_f64().is_some());
    assert_eq!(body["cores"].as_array().map(Vec::len), Some(2));
    assert!(body["total"]["user_ticks"].as_u64().is_some());
}

#[tokio::test]
async fn memory_returns_metrics_from_fixture() {
    let (status, body) = common::get("/v1/system/memory").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total_bytes"], 16_384_000_u64 * 1024);
    assert_eq!(body["used_bytes"], (16_384_000_u64 - 12_000_000) * 1024);
    assert_eq!(body["swap_used_bytes"], 0);
}

#[tokio::test]
async fn cpu_is_unavailable_when_proc_is_missing() {
    let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/");
    let (status, body) = common::get_for(common::app_with(paths), "/v1/system/cpu").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn memory_is_unavailable_when_meminfo_is_missing() {
    let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/");
    let (status, body) = common::get_for(common::app_with(paths), "/v1/system/memory").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
}
