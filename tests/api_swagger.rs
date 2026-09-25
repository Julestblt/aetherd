//! Integration tests for the feature-gated Swagger UI.

#![cfg(feature = "swagger-ui")]

mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn swagger_ui_serves_html() {
    let (status, content_type, body) = common::get_text("/swagger-ui/").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("text/html")),
        "unexpected content type: {content_type:?}"
    );
    assert!(body.to_lowercase().contains("swagger"));
}
