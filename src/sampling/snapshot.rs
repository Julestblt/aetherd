use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::system::cpu::CpuMetrics;
use crate::system::disks::DisksMetrics;
use crate::system::host::HostMetrics;
use crate::system::load::LoadMetrics;
use crate::system::memory::MemoryMetrics;
use crate::system::network::NetworkMetrics;
use crate::system::uptime::UptimeMetrics;

/// A metric that may or may not be available on the current host.
///
/// Aggregated responses use this so a single failing collector never fails the
/// whole payload.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Section<T> {
    /// The metric was collected.
    Available {
        /// The collected metric.
        value: T,
    },
    /// The metric could not be collected on this host.
    Unavailable {
        /// Why the metric is unavailable.
        reason: String,
    },
}

/// One complete point-in-time observation of the system.
///
/// Every section is independent: a section that could not be collected is
/// marked unavailable with a reason, and never replaced by a synthetic value.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = SystemOverview)]
pub struct SystemSnapshot {
    /// RFC3339 UTC timestamp of this sample.
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) collected_at: OffsetDateTime,
    /// Host metadata.
    pub(crate) host: Section<HostMetrics>,
    /// CPU metrics.
    pub(crate) cpu: Section<CpuMetrics>,
    /// Memory metrics.
    pub(crate) memory: Section<MemoryMetrics>,
    /// Load average.
    pub(crate) load: Section<LoadMetrics>,
    /// System uptime.
    pub(crate) uptime: Section<UptimeMetrics>,
    /// Filesystem metrics.
    pub(crate) disks: Section<DisksMetrics>,
    /// Network metrics.
    pub(crate) network: Section<NetworkMetrics>,
}
