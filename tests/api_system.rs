//! Integration tests for the `/v1/system` CPU and memory endpoints.

mod common;

use std::sync::Arc;

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

#[tokio::test]
async fn host_returns_fixture_metadata() {
    let (status, body) = common::get("/v1/system/host").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hostname"], "aetherd-test-host");
    assert_eq!(body["kernel_release"], "6.8.0-aetherd");
    assert_eq!(body["os"]["id"], "aetherd-test");
    assert_eq!(body["os"]["pretty_name"], "Aetherd Test Linux 1.0");
    assert!(body["boot_time"].is_string());
}

#[tokio::test]
async fn load_returns_fixture_metrics() {
    let (status, body) = common::get("/v1/system/load").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["load1"], 1.5);
    assert_eq!(body["runnable"], 2);
    assert_eq!(body["total_processes"], 1234);
}

#[tokio::test]
async fn uptime_returns_fixture_metrics() {
    let (status, body) = common::get("/v1/system/uptime").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["uptime_seconds"], 12345.67);
    assert_eq!(body["idle_seconds"], 89012.34);
}

#[tokio::test]
async fn load_is_unavailable_when_loadavg_is_missing() {
    let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/");
    let (status, body) = common::get_for(common::app_with(paths), "/v1/system/load").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
}

#[tokio::test]
async fn host_tolerates_missing_metadata() {
    let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/nonexistent/root");
    let (status, body) = common::get_for(common::app_with(paths), "/v1/system/host").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hostname"], serde_json::Value::Null);
    assert_eq!(body["os"], serde_json::Value::Null);
    assert!(body["architecture"].is_string());
}

#[tokio::test]
async fn disks_exclude_pseudo_filesystems() {
    let (status, body) = common::get("/v1/system/disks").await;

    assert_eq!(status, StatusCode::OK);
    let filesystems = body["filesystems"].as_array().expect("filesystems array");
    assert_eq!(filesystems.len(), 2);
    assert_eq!(filesystems[0]["mount_point"], "/");
    assert_eq!(filesystems[1]["mount_point"], "/data volume");
    assert_eq!(filesystems[0]["usage"]["total_bytes"], 1000);
}

#[tokio::test]
async fn disks_mark_usage_unavailable_without_failing() {
    let mount_stats = Arc::new(common::FakeMountStats { usage: None });
    let router = common::app_with_mount_stats(common::fixture_paths(), mount_stats);

    let (status, body) = common::get_for(router, "/v1/system/disks").await;

    assert_eq!(status, StatusCode::OK);
    let filesystems = body["filesystems"].as_array().expect("filesystems array");
    assert_eq!(filesystems.len(), 2);
    assert_eq!(filesystems[0]["usage"], serde_json::Value::Null);
}

#[tokio::test]
async fn disks_are_unavailable_when_mounts_is_missing() {
    let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/");
    let (status, body) = common::get_for(common::app_with(paths), "/v1/system/disks").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
}

#[tokio::test]
async fn network_returns_fixture_interfaces() {
    let (status, body) = common::get("/v1/system/network").await;

    assert_eq!(status, StatusCode::OK);
    let interfaces = body["interfaces"].as_array().expect("interfaces array");
    assert_eq!(interfaces.len(), 2);
    assert_eq!(interfaces[1]["name"], "eth0");
    assert_eq!(interfaces[1]["rx_bytes"], 5000);
    assert_eq!(interfaces[1]["tx_dropped"], 4);
}

#[tokio::test]
async fn network_is_unavailable_when_dev_is_missing() {
    let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/");
    let (status, body) = common::get_for(common::app_with(paths), "/v1/system/network").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
}
