use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};

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

const STREAM_EVENT_NAME: &str = "system";
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

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

#[utoipa::path(
    get,
    path = "/stream",
    tag = "system",
    responses((
        status = 200,
        description = "Server-Sent Events stream. Each event is named `system` and its data is the latest SystemSnapshot serialized as JSON. The first event carries the current snapshot; later events arrive as new samples are published."
    ))
)]
pub(crate) async fn stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut snapshots = state.subscribe();
    let mut shutdown = state.shutdown_receiver();

    let stream = async_stream::stream! {
        loop {
            if let Some(event) = current_event(&mut snapshots) {
                yield event;
            }

            tokio::select! {
                changed = snapshots.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                () = shutdown.recv() => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(KEEP_ALIVE_INTERVAL)
            .text("keep-alive"),
    )
}

fn current_event(
    snapshots: &mut tokio::sync::watch::Receiver<Option<Arc<SystemSnapshot>>>,
) -> Option<Result<Event, Infallible>> {
    let guard = snapshots.borrow_and_update();
    let snapshot = guard.as_ref()?;
    match Event::default()
        .event(STREAM_EVENT_NAME)
        .json_data(&**snapshot)
    {
        Ok(event) => Some(Ok(event)),
        Err(error) => {
            tracing::warn!(error = %error, "failed to serialize snapshot for SSE");
            None
        }
    }
}
