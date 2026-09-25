//! API regression tests: error behavior, malformed inputs, and response
//! contract snapshots.

mod common;

use axum::http::StatusCode;

use aetherd::SystemPaths;

fn broken_paths() -> SystemPaths {
    let root = common::fixture_root();
    SystemPaths::new(root.join("broken/proc"), root.join("broken/sys"), root)
}

#[tokio::test]
async fn unsupported_method_returns_structured_405() {
    let (status, body) = common::post("/health").await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["error"]["code"], "method_not_allowed");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn malformed_cpu_fixture_returns_structured_503() {
    let (status, body) = common::get_for(common::app_with(broken_paths()), "/v1/system/cpu").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
}

#[tokio::test]
async fn malformed_memory_fixture_returns_structured_503() {
    let (status, body) =
        common::get_for(common::app_with(broken_paths()), "/v1/system/memory").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
}

#[tokio::test]
async fn malformed_section_does_not_fail_overview() {
    let (status, body) = common::get_for(common::app_with(broken_paths()), "/v1/system").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cpu"]["status"], "unavailable");
    assert_eq!(body["memory"]["status"], "unavailable");
}

#[tokio::test]
async fn cpu_response_matches_snapshot() {
    let (_, body) = common::get("/v1/system/cpu").await;
    insta::assert_json_snapshot!(body);
}

#[tokio::test]
async fn disks_response_matches_snapshot() {
    let (_, body) = common::get("/v1/system/disks").await;
    insta::assert_json_snapshot!(body);
}
