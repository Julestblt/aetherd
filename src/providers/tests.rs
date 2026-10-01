use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;
use crate::app::{AppState, build_router};
use crate::config::ProvidersConfig;
use crate::sampling::Shutdown;
use crate::system::SystemPaths;

#[derive(Debug)]
struct FakeProvider {
    id: &'static str,
    succeeds: bool,
}

impl UsageProvider for FakeProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn collect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderUsage, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            if !self.succeeds {
                return Err(ProviderError::Request);
            }
            Ok(ProviderUsage {
                provider_id: self.id.to_owned(),
                display_name: self.id.to_owned(),
                account_label: None,
                windows: Vec::new(),
                totals: None,
                models: None,
            })
        })
    }
}

async fn get_json(router: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, serde_json::from_slice(&bytes).expect("json"))
}

#[tokio::test]
async fn configured_providers_are_disabled_by_default() {
    let state = AppState::new(SystemPaths::default()).with_providers(ProvidersConfig::default());
    let router = build_router(state);
    let (status, providers) = get_json(router.clone(), "/v1/providers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(providers[0]["status"], "disabled");
    assert_eq!(providers[1]["status"], "disabled");
    let (status, usage) = get_json(router, "/v1/usage").await;
    assert_eq!(status, StatusCode::OK);
    assert!(usage["collected_at"].is_null());
    assert!(usage["providers"].as_array().expect("providers").is_empty());
}

#[test]
fn missing_codex_auth_and_opencode_key_leave_providers_unavailable() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mut config = ProvidersConfig::default();
    config.codex.enabled = true;
    config.codex.auth_file = Some(directory.path().join("missing-codex.json"));
    config.opencode.enabled = true;
    let state = AppState::new(SystemPaths::default()).with_providers(config);
    assert!(spawn_provider_refreshers(&state).is_empty());
    let statuses = state.provider_cache().statuses();
    assert_eq!(statuses[0].status, ProviderState::Unavailable);
    assert_eq!(statuses[1].status, ProviderState::Unavailable);
    assert!(
        statuses[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("no auth file"))
    );
    assert!(
        statuses[1]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("API key is not configured"))
    );
}

#[tokio::test]
async fn provider_failures_are_isolated_in_both_directions() {
    for (codex_ok, go_ok) in [(false, true), (true, false)] {
        let mut config = ProvidersConfig::default();
        config.codex.enabled = true;
        config.opencode.enabled = true;
        config.opencode.api_key = crate::config::SecretString::new("oc_sk_test-secret");
        let state = AppState::new(SystemPaths::default()).with_providers(config);
        let at = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("time");
        for (id, succeeds) in [("codex", codex_ok), ("opencode-go", go_ok)] {
            state
                .provider_cache()
                .publish(id, FakeProvider { id, succeeds }.collect().await, at);
        }
        let router = build_router(state.clone());
        let (status, providers) = get_json(router.clone(), "/v1/providers").await;
        assert_eq!(status, StatusCode::OK);
        assert!(!providers.to_string().contains("oc_sk_test-secret"));
        assert_eq!(
            providers[0]["status"],
            if codex_ok { "available" } else { "unavailable" }
        );
        assert_eq!(
            providers[1]["status"],
            if go_ok { "available" } else { "unavailable" }
        );
        let (status, usage) = get_json(router, "/v1/usage").await;
        assert_eq!(status, StatusCode::OK);
        assert!(!usage.to_string().contains("oc_sk_test-secret"));
        let entries = usage["providers"].as_array().expect("providers");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]["provider_id"],
            if codex_ok { "codex" } else { "opencode-go" }
        );
        assert_eq!(usage["collected_at"], "2023-11-14T22:13:20Z");
    }
}

#[tokio::test]
async fn failed_refresh_keeps_the_last_successful_value() {
    let cache = ProviderCache::new();
    cache.configure("codex", "Codex", true, None);
    let at = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("time");
    let provider = FakeProvider {
        id: "codex",
        succeeds: true,
    };
    cache.publish("codex", provider.collect().await, at);
    let before = serde_json::to_value(cache.usage()).expect("json");
    cache.publish(
        "codex",
        Err(ProviderError::Request),
        at + time::Duration::minutes(1),
    );
    assert_eq!(serde_json::to_value(cache.usage()).expect("json"), before);
    assert_eq!(cache.statuses()[0].status, ProviderState::Unavailable);
    assert_eq!(cache.statuses()[0].last_updated_at, Some(at));
}

#[tokio::test(start_paused = true)]
async fn refresh_task_runs_independently_and_stops_on_shutdown() {
    let cache = ProviderCache::new();
    cache.configure("codex", "Codex", true, None);
    let (shutdown, _) = tokio::sync::watch::channel(false);
    let task = refresh::spawn(
        Arc::new(FakeProvider {
            id: "codex",
            succeeds: true,
        }),
        cache.clone(),
        Duration::from_secs(60),
        Shutdown::receiver(&shutdown),
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(cache.usage().providers.len(), 1);
    shutdown.send_replace(true);
    task.await.expect("task joins");
}
