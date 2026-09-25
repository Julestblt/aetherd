use axum::Json;
use axum::extract::State;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::app::AppState;

/// Liveness response for `GET /health`.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct HealthResponse {
    /// Always `ok` while the process is serving requests.
    pub status: HealthStatus,
    /// Daemon version taken from the crate metadata.
    pub version: String,
    /// RFC3339 UTC timestamp of daemon start.
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    /// Seconds elapsed since the daemon started.
    pub uptime_seconds: f64,
    /// RFC3339 UTC timestamp of this response.
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
}

/// Liveness state of the daemon.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HealthStatus {
    /// The daemon is running.
    Ok,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses((status = 200, description = "Daemon is alive", body = HealthResponse))
)]
pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let collected_at = OffsetDateTime::now_utc();
    let uptime_seconds = (collected_at - state.started_at).as_seconds_f64().max(0.0);
    Json(HealthResponse {
        status: HealthStatus::Ok,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at: state.started_at,
        uptime_seconds,
        collected_at,
    })
}
