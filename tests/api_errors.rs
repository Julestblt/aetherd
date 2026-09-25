//! Integration tests for structured error responses.

mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn unknown_route_returns_structured_not_found() {
    let (status, body) = common::get("/does-not-exist").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(body["error"]["message"].is_string());
}
