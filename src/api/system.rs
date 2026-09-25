use axum::Json;
use axum::extract::State;

use crate::api::error::{ApiError, ErrorResponse};
use crate::app::AppState;
use crate::system::cpu::{CpuCollector, CpuMetrics};
use crate::system::host::{HostCollector, HostMetrics};
use crate::system::load::{LoadCollector, LoadMetrics};
use crate::system::memory::{MemoryCollector, MemoryMetrics};
use crate::system::uptime::{UptimeCollector, UptimeMetrics};
use crate::system::{SystemCollector, SystemPaths};

#[utoipa::path(
    get,
    path = "/cpu",
    tag = "system",
    responses(
        (status = 200, description = "CPU metrics", body = CpuMetrics),
        (status = 503, description = "CPU metrics unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn cpu(State(state): State<AppState>) -> Result<Json<CpuMetrics>, ApiError> {
    probe(&CpuCollector, &state.paths).map(Json)
}

#[utoipa::path(
    get,
    path = "/memory",
    tag = "system",
    responses(
        (status = 200, description = "Memory metrics", body = MemoryMetrics),
        (status = 503, description = "Memory metrics unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn memory(State(state): State<AppState>) -> Result<Json<MemoryMetrics>, ApiError> {
    probe(&MemoryCollector, &state.paths).map(Json)
}

#[utoipa::path(
    get,
    path = "/host",
    tag = "system",
    responses((status = 200, description = "Host information", body = HostMetrics))
)]
pub(crate) async fn host(State(state): State<AppState>) -> Result<Json<HostMetrics>, ApiError> {
    probe(&HostCollector, &state.paths).map(Json)
}

#[utoipa::path(
    get,
    path = "/load",
    tag = "system",
    responses(
        (status = 200, description = "Load average", body = LoadMetrics),
        (status = 503, description = "Load average unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn load(State(state): State<AppState>) -> Result<Json<LoadMetrics>, ApiError> {
    probe(&LoadCollector, &state.paths).map(Json)
}

#[utoipa::path(
    get,
    path = "/uptime",
    tag = "system",
    responses(
        (status = 200, description = "System uptime", body = UptimeMetrics),
        (status = 503, description = "Uptime unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn uptime(State(state): State<AppState>) -> Result<Json<UptimeMetrics>, ApiError> {
    probe(&UptimeCollector, &state.paths).map(Json)
}

fn probe<C: SystemCollector>(collector: &C, paths: &SystemPaths) -> Result<C::Metric, ApiError> {
    collector.collect(paths).map_err(|error| {
        tracing::warn!(collector = collector.name(), error = %error, "collector failed");
        ApiError::Unavailable(error.to_string())
    })
}
