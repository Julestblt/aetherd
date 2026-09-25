use std::sync::Arc;

use time::OffsetDateTime;

use super::snapshot::{Section, SystemSnapshot};
use crate::system::cpu::CpuCollector;
use crate::system::disks::DiskCollector;
use crate::system::host::HostCollector;
use crate::system::load::LoadCollector;
use crate::system::memory::MemoryCollector;
use crate::system::network::NetworkCollector;
use crate::system::uptime::UptimeCollector;
use crate::system::{MountStats, SystemCollector, SystemPaths};

/// Builds [`SystemSnapshot`]s from configured telemetry sources.
///
/// The builder is synchronous and deterministic given a filesystem state, which
/// keeps sampling testable with fixtures and injected disk statistics.
#[derive(Debug)]
pub struct SnapshotBuilder {
    paths: SystemPaths,
    mount_stats: Arc<dyn MountStats>,
}

impl SnapshotBuilder {
    /// Creates a builder reading from the given roots and disk-statistics
    /// source.
    #[must_use]
    pub fn new(paths: SystemPaths, mount_stats: Arc<dyn MountStats>) -> Self {
        Self { paths, mount_stats }
    }

    /// Collects one snapshot as of `collected_at`.
    #[must_use]
    pub fn collect(&self, collected_at: OffsetDateTime) -> SystemSnapshot {
        let disks = DiskCollector::new(Arc::clone(&self.mount_stats));

        SystemSnapshot {
            collected_at,
            host: probe_section(&HostCollector, &self.paths),
            cpu: probe_section(&CpuCollector, &self.paths),
            memory: probe_section(&MemoryCollector, &self.paths),
            load: probe_section(&LoadCollector, &self.paths),
            uptime: probe_section(&UptimeCollector, &self.paths),
            disks: probe_section(&disks, &self.paths),
            network: probe_section(&NetworkCollector, &self.paths),
        }
    }
}

fn probe_section<C: SystemCollector>(collector: &C, paths: &SystemPaths) -> Section<C::Metric> {
    match collector.collect(paths) {
        Ok(value) => Section::Available { value },
        Err(error) => {
            tracing::warn!(
                collector = collector.name(),
                error = %error,
                "collector unavailable in snapshot"
            );
            Section::Unavailable {
                reason: error.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use super::*;
    use crate::system::DiskUsage;

    #[derive(Debug)]
    struct FixedStats;

    impl MountStats for FixedStats {
        fn usage(&self, _path: &Path) -> Result<DiskUsage, io::Error> {
            Ok(DiskUsage {
                total_bytes: 10,
                free_bytes: 5,
                available_bytes: 4,
                used_bytes: 5,
                used_percent: 50.0,
            })
        }
    }

    fn fixture_paths() -> SystemPaths {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        SystemPaths::new(root.join("proc"), root.join("sys"), root)
    }

    fn sample_time() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid timestamp")
    }

    #[test]
    fn collects_every_section_from_fixtures() {
        let builder = SnapshotBuilder::new(fixture_paths(), Arc::new(FixedStats));
        let snapshot = builder.collect(sample_time());

        assert_eq!(snapshot.collected_at, sample_time());
        assert!(matches!(snapshot.host, Section::Available { .. }));
        assert!(matches!(snapshot.cpu, Section::Available { .. }));
        assert!(matches!(snapshot.memory, Section::Available { .. }));
        assert!(matches!(snapshot.load, Section::Available { .. }));
        assert!(matches!(snapshot.uptime, Section::Available { .. }));
        assert!(matches!(snapshot.disks, Section::Available { .. }));
        assert!(matches!(snapshot.network, Section::Available { .. }));
    }

    #[test]
    fn unavailable_sources_are_marked_without_failing_the_snapshot() {
        let paths = SystemPaths::new("/nonexistent/proc", "/nonexistent/sys", "/nonexistent/root");
        let builder = SnapshotBuilder::new(paths, Arc::new(FixedStats));
        let snapshot = builder.collect(sample_time());

        assert!(matches!(snapshot.cpu, Section::Unavailable { .. }));
        assert!(matches!(snapshot.memory, Section::Unavailable { .. }));
        assert!(matches!(snapshot.host, Section::Available { .. }));
    }
}
