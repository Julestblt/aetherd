use utoipa::OpenApi;

use crate::api::error::{ErrorCode, ErrorDetail, ErrorResponse};
use crate::api::health::{HealthResponse, HealthStatus};
use crate::sampling::{Section, SystemSnapshot};
use crate::system::cpu::{CpuCore, CpuMetrics, CpuTimes};
use crate::system::disks::{DiskUsage, DisksMetrics, FilesystemMetrics};
use crate::system::host::{HostMetrics, OsRelease};
use crate::system::load::LoadMetrics;
use crate::system::memory::MemoryMetrics;
use crate::system::network::{NetworkInterface, NetworkMetrics};
use crate::system::uptime::UptimeMetrics;

/// `OpenAPI` document generated from the Rust types and handler annotations.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "aetherd API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Linux system telemetry and, later, normalized AI provider usage.",
        license(name = "MIT OR Apache-2.0")
    ),
    paths(
        crate::api::health::health,
        crate::api::system::overview,
        crate::api::system::cpu,
        crate::api::system::memory,
        crate::api::system::host,
        crate::api::system::load,
        crate::api::system::uptime,
        crate::api::system::disks,
        crate::api::system::network
    ),
    components(schemas(
        HealthResponse,
        HealthStatus,
        SystemSnapshot,
        Section<HostMetrics>,
        Section<CpuMetrics>,
        Section<MemoryMetrics>,
        Section<LoadMetrics>,
        Section<UptimeMetrics>,
        Section<DisksMetrics>,
        Section<NetworkMetrics>,
        CpuMetrics,
        CpuTimes,
        CpuCore,
        MemoryMetrics,
        HostMetrics,
        OsRelease,
        LoadMetrics,
        UptimeMetrics,
        DisksMetrics,
        FilesystemMetrics,
        DiskUsage,
        NetworkMetrics,
        NetworkInterface,
        ErrorResponse,
        ErrorDetail,
        ErrorCode
    ))
)]
pub(crate) struct ApiDoc;
