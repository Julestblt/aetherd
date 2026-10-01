use utoipa::OpenApi;

use crate::api::error::{ErrorCode, ErrorDetail, ErrorResponse};
use crate::api::health::{HealthResponse, HealthStatus};
use crate::providers::{
    ModelUsage, ProviderState, ProviderStatus, ProviderUsage, UsageSnapshot, UsageWindow,
    WindowKind,
};
use crate::sampling::{Section, SystemSnapshot};
use crate::system::cpu::{CpuCore, CpuMetrics, CpuTimes};
use crate::system::disks::{DiskUsage, DisksMetrics, FilesystemMetrics};
use crate::system::host::{HostMetrics, OsRelease};
use crate::system::load::LoadMetrics;
use crate::system::memory::MemoryMetrics;
use crate::system::network::{NetworkInterface, NetworkMetrics};
use crate::system::uptime::UptimeMetrics;
use crate::tailscale::{TailscaleDevice, TailscaleSnapshot};

/// `OpenAPI` document generated from the Rust types and handler annotations.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "aetherd API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Linux system telemetry and normalized AI provider usage.",
        license(name = "MIT OR Apache-2.0")
    ),
    paths(crate::api::health::health, crate::api::providers::providers, crate::api::providers::usage),
    components(schemas(
        HealthResponse,
        HealthStatus,
        ProviderStatus,
        ProviderState,
        UsageSnapshot,
        ProviderUsage,
        UsageWindow,
        WindowKind,
        ModelUsage,
        SystemSnapshot,
        Section<HostMetrics>,
        Section<CpuMetrics>,
        Section<MemoryMetrics>,
        Section<LoadMetrics>,
        Section<UptimeMetrics>,
        Section<DisksMetrics>,
        Section<NetworkMetrics>,
        Section<TailscaleSnapshot>,
        TailscaleSnapshot,
        TailscaleDevice,
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
