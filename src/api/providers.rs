use axum::Json;
use axum::extract::State;

use crate::app::AppState;
use crate::providers::{ProviderStatus, UsageSnapshot};

/// Returns current status for configured AI usage providers.
#[utoipa::path(get, path = "/v1/providers", responses((status = 200, body = Vec<ProviderStatus>)))]
pub(crate) async fn providers(State(state): State<AppState>) -> Json<Vec<ProviderStatus>> {
    Json(state.provider_cache().statuses())
}

/// Returns the last successful normalized usage for each provider.
#[utoipa::path(get, path = "/v1/usage", responses((status = 200, body = UsageSnapshot)))]
pub(crate) async fn usage(State(state): State<AppState>) -> Json<UsageSnapshot> {
    Json(state.provider_cache().usage())
}
