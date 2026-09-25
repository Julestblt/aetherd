//! Shared helpers for HTTP integration tests.

#![allow(
    clippy::expect_used,
    reason = "integration-test helpers fail loudly when request setup breaks"
)]

use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use aetherd::{AppState, SystemPaths, build_router};

pub(crate) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub(crate) fn fixture_paths() -> SystemPaths {
    let root = fixture_root();
    SystemPaths::new(root.join("proc"), root.join("sys"), root)
}

pub(crate) fn app() -> Router {
    app_with(fixture_paths())
}

pub(crate) fn app_with(paths: SystemPaths) -> Router {
    build_router(AppState::new(paths))
}

pub(crate) async fn get(uri: &str) -> (StatusCode, serde_json::Value) {
    get_for(app(), uri).await
}

pub(crate) async fn get_for(router: Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = router
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
