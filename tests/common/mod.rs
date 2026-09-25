//! Shared helpers for HTTP integration tests.

#![allow(
    clippy::expect_used,
    reason = "integration-test helpers fail loudly when request setup breaks"
)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use aetherd::{AppState, build_router};

pub(crate) fn app() -> Router {
    build_router(AppState::new())
}

pub(crate) async fn get(uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collected")
        .to_bytes();
    let json = serde_json::from_slice(&bytes).expect("json body");
    (status, json)
}
