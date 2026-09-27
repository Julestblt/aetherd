//! Integration tests for optional Tailscale tailnet telemetry.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;

use aetherd::{AppState, SecretString, TailscaleConfig};

fn incomplete_state() -> AppState {
    let config = TailscaleConfig {
        api_key: SecretString::default(),
        ..common::tailscale_config()
    };
    let state = AppState::with_mount_stats(
        common::fixture_paths(),
        Arc::new(common::FakeMountStats::default()),
    )
    .with_tailscale(config);
    let snapshot = state.snapshot_builder().collect(common::sample_time());
    state.publish(snapshot);
    state
}

#[tokio::test]
async fn tailscale_returns_cached_devices() {
    let (status, body) =
        common::get_for(common::app_with_tailscale(), "/v1/system/tailscale").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tailnet"], "example.ts.net");
    assert_eq!(body["collected_at"], "2023-11-14T22:13:20Z");
    let devices = body["devices"].as_array().expect("devices array");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["id"], "node-1");
    assert_eq!(devices[0]["hostname"], "homelab");
    assert_eq!(devices[0]["dns_name"], "homelab.example.ts.net");
    assert_eq!(devices[0]["os"], "linux");
    assert_eq!(devices[0]["addresses"][0], "100.64.0.1");
    assert_eq!(devices[0]["online"], true);
    assert_eq!(devices[0]["authorized"], true);
    assert_eq!(devices[0]["last_seen"], "2023-11-14T22:13:20Z");
    assert_eq!(devices[0]["tags"][0], "tag:homelab");
}

#[tokio::test]
async fn tailscale_is_disabled_by_default() {
    let (status, body) = common::get("/v1/system/tailscale").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
    assert_eq!(
        body["error"]["message"],
        "tailscale integration is disabled"
    );
}

#[tokio::test]
async fn tailscale_is_not_ready_before_the_first_snapshot() {
    let state = common::unpublished_state(
        common::fixture_paths(),
        Arc::new(common::FakeMountStats::default()),
    );
    let (status, body) =
        common::get_for(aetherd::build_router(state), "/v1/system/tailscale").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "not_ready");
}

#[tokio::test]
async fn incomplete_configuration_reports_an_unavailable_reason() {
    let (status, body) = common::get_for(
        aetherd::build_router(incomplete_state()),
        "/v1/system/tailscale",
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "unavailable");
    assert!(
        body["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("API key"))
    );
}

#[tokio::test]
async fn overview_includes_the_tailscale_section() {
    let (status, body) = common::get_for(common::app_with_tailscale(), "/v1/system").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tailscale"]["status"], "available");
    assert_eq!(
        body["tailscale"]["value"]["devices"][0]["hostname"],
        "homelab"
    );
    assert_eq!(body["cpu"]["status"], "available");
}

#[tokio::test]
async fn overview_reports_tailscale_as_unavailable_when_disabled() {
    let (status, body) = common::get("/v1/system").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tailscale"]["status"], "unavailable");
    assert_eq!(
        body["tailscale"]["reason"],
        "tailscale integration is disabled"
    );
    assert_eq!(body["cpu"]["status"], "available");
}

#[tokio::test]
async fn cached_tailscale_value_survives_unrelated_system_sampling() {
    let state = common::state_with_tailscale();
    let router = aetherd::build_router(state.clone());

    let first = common::get_for(router.clone(), "/v1/system/tailscale").await;
    assert_eq!(first.1["devices"][0]["hostname"], "homelab");

    let later = state
        .snapshot_builder()
        .collect(common::sample_time() + time::Duration::seconds(30));
    state.publish(later);

    let (status, body) = common::get_for(router, "/v1/system/tailscale").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["devices"][0]["hostname"], "homelab");
    assert_eq!(body["collected_at"], "2023-11-14T22:13:20Z");
}

#[tokio::test]
async fn the_api_key_is_never_serialized() {
    let state = common::state_with_tailscale();
    let router = aetherd::build_router(state.clone());

    assert!(!format!("{state:?}").contains(common::TAILSCALE_API_KEY));

    for uri in ["/v1/system", "/v1/system/tailscale", "/openapi.json"] {
        let (_, _, text) = common::get_text_for(router.clone(), uri).await;
        assert!(
            !text.contains(common::TAILSCALE_API_KEY),
            "{uri} must not expose the Tailscale API key"
        );
    }
}
