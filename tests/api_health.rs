//! Integration tests for `GET /health`.

mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn health_reports_ok_and_version() {
    let (status, body) = common::get("/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["uptime_seconds"].as_f64().is_some_and(|v| v >= 0.0));

    for field in ["started_at", "collected_at"] {
        let value = body[field].as_str().expect("timestamp is a string");
        assert!(
            value.contains('T'),
            "{field} should be RFC3339, got {value:?}"
        );
    }
}
