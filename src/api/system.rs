use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::api::error::{ApiError, ErrorResponse};
use crate::app::AppState;
use crate::sampling::{Section, SystemSnapshot};
use crate::system::cpu::CpuMetrics;
use crate::system::disks::DisksMetrics;
use crate::system::host::HostMetrics;
use crate::system::load::LoadMetrics;
use crate::system::memory::MemoryMetrics;
use crate::system::network::NetworkMetrics;
use crate::system::uptime::UptimeMetrics;

#[utoipa::path(
    get,
    path = "/",
    tag = "system",
    responses(
        (status = 200, description = "Latest system snapshot; every section reports its own availability", body = SystemSnapshot),
        (status = 503, description = "No snapshot collected yet", body = ErrorResponse)
    )
)]
pub(crate) async fn overview(
    State(state): State<AppState>,
) -> Result<Json<Arc<SystemSnapshot>>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    Ok(Json(Arc::clone(snapshot)))
}

#[utoipa::path(
    get,
    path = "/cpu",
    tag = "system",
    responses(
        (status = 200, description = "CPU metrics", body = CpuMetrics),
        (status = 503, description = "Snapshot not ready or CPU metrics unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn cpu(State(state): State<AppState>) -> Result<Json<CpuMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.cpu).map(Json)
}

#[utoipa::path(
    get,
    path = "/memory",
    tag = "system",
    responses(
        (status = 200, description = "Memory metrics", body = MemoryMetrics),
        (status = 503, description = "Snapshot not ready or memory metrics unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn memory(State(state): State<AppState>) -> Result<Json<MemoryMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.memory).map(Json)
}

#[utoipa::path(
    get,
    path = "/host",
    tag = "system",
    responses(
        (status = 200, description = "Host information", body = HostMetrics),
        (status = 503, description = "Snapshot not ready or host information unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn host(State(state): State<AppState>) -> Result<Json<HostMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.host).map(Json)
}

#[utoipa::path(
    get,
    path = "/load",
    tag = "system",
    responses(
        (status = 200, description = "Load average", body = LoadMetrics),
        (status = 503, description = "Snapshot not ready or load average unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn load(State(state): State<AppState>) -> Result<Json<LoadMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.load).map(Json)
}

#[utoipa::path(
    get,
    path = "/uptime",
    tag = "system",
    responses(
        (status = 200, description = "System uptime", body = UptimeMetrics),
        (status = 503, description = "Snapshot not ready or uptime unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn uptime(State(state): State<AppState>) -> Result<Json<UptimeMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.uptime).map(Json)
}

#[utoipa::path(
    get,
    path = "/disks",
    tag = "system",
    responses(
        (status = 200, description = "Filesystem metrics, excluding pseudo-filesystems", body = DisksMetrics),
        (status = 503, description = "Snapshot not ready or filesystem metrics unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn disks(State(state): State<AppState>) -> Result<Json<DisksMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.disks).map(Json)
}

#[utoipa::path(
    get,
    path = "/network",
    tag = "system",
    responses(
        (status = 200, description = "Network interface counters", body = NetworkMetrics),
        (status = 503, description = "Snapshot not ready or network metrics unavailable", body = ErrorResponse)
    )
)]
pub(crate) async fn network(
    State(state): State<AppState>,
) -> Result<Json<NetworkMetrics>, ApiError> {
    let guard = state.snapshot();
    let snapshot = guard.as_ref().ok_or(ApiError::NotReady)?;
    section_value(&snapshot.network).map(Json)
}

fn section_value<T: Clone>(section: &Section<T>) -> Result<T, ApiError> {
    match section {
        Section::Available { value } => Ok(value.clone()),
        Section::Unavailable { reason } => Err(ApiError::Unavailable(reason.clone())),
    }
}
