//! Shared helpers for HTTP integration tests.

#![allow(
    clippy::expect_used,
    reason = "integration-test helpers fail loudly when request setup breaks"
)]
#![allow(
    dead_code,
    reason = "each integration test crate compiles the shared helpers it needs"
)]

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use aetherd::{AppState, DiskUsage, MountStats, SystemPaths, build_router};

pub(crate) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub(crate) fn fixture_paths() -> SystemPaths {
    let root = fixture_root();
    SystemPaths::new(root.join("proc"), root.join("sys"), root)
}

#[derive(Debug, Clone)]
pub(crate) struct FakeMountStats {
    pub(crate) usage: Option<DiskUsage>,
}

impl Default for FakeMountStats {
    fn default() -> Self {
        Self {
            usage: Some(DiskUsage {
                total_bytes: 1000,
                free_bytes: 400,
                available_bytes: 300,
                used_bytes: 600,
                used_percent: 66.66,
            }),
        }
    }
}

impl MountStats for FakeMountStats {
    fn usage(&self, _path: &Path) -> Result<DiskUsage, io::Error> {
        match &self.usage {
            Some(usage) => Ok(usage.clone()),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "no stats")),
        }
    }
}

pub(crate) fn app() -> Router {
    app_with(fixture_paths())
}

pub(crate) fn app_with(paths: SystemPaths) -> Router {
    app_with_mount_stats(paths, Arc::new(FakeMountStats::default()))
}

pub(crate) fn app_with_mount_stats(paths: SystemPaths, mount_stats: Arc<dyn MountStats>) -> Router {
    build_router(AppState::with_mount_stats(paths, mount_stats))
}

pub(crate) async fn get(uri: &str) -> (StatusCode, serde_json::Value) {
    send_for(app(), Method::GET, uri).await
}

pub(crate) async fn get_for(router: Router, uri: &str) -> (StatusCode, serde_json::Value) {
    send_for(router, Method::GET, uri).await
}

pub(crate) async fn post(uri: &str) -> (StatusCode, serde_json::Value) {
    send_for(app(), Method::POST, uri).await
}

pub(crate) async fn send_for(
    router: Router,
    method: Method,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method(method)
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
