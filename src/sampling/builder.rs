use std::sync::Arc;

use time::OffsetDateTime;

use super::delta;
use super::snapshot::{Section, SystemSnapshot};
use crate::system::cpu::{CpuCollector, CpuMetrics};
use crate::system::disks::DiskCollector;
use crate::system::host::HostCollector;
use crate::system::load::LoadCollector;
use crate::system::memory::MemoryCollector;
use crate::system::network::{NetworkCollector, NetworkMetrics};
use crate::system::uptime::UptimeCollector;
use crate::system::{MountStats, SystemCollector, SystemPaths};

/// Builds [`SystemSnapshot`]s from configured telemetry sources.
///
/// The builder keeps the previous CPU and network readings so interval-derived
/// metrics can be computed on each sample. It is synchronous and deterministic
/// given a filesystem state, which keeps sampling testable with fixtures and
/// injected disk statistics.
#[derive(Debug)]
pub struct SnapshotBuilder {
    paths: SystemPaths,
    mount_stats: Arc<dyn MountStats>,
    cpu_baseline: Option<CpuMetrics>,
    network_baseline: Option<(OffsetDateTime, NetworkMetrics)>,
}

impl SnapshotBuilder {
    /// Creates a builder reading from the given roots and disk-statistics
    /// source.
    #[must_use]
    pub fn new(paths: SystemPaths, mount_stats: Arc<dyn MountStats>) -> Self {
        Self {
            paths,
            mount_stats,
            cpu_baseline: None,
            network_baseline: None,
        }
    }

    /// Collects one snapshot as of `collected_at`, deriving interval metrics
    /// from the previous successful sample where available.
    pub fn collect(&mut self, collected_at: OffsetDateTime) -> SystemSnapshot {
        let disks = DiskCollector::new(Arc::clone(&self.mount_stats));
        let mut cpu = probe_section(&CpuCollector, &self.paths);
        let mut network = probe_section(&NetworkCollector, &self.paths);

        if let Section::Available { value } = &mut cpu {
            if let Some(previous) = &self.cpu_baseline {
                delta::fill_cpu_interval(value, previous);
            }
            self.cpu_baseline = Some(value.clone());
        }

        if let Section::Available { value } = &mut network {
            if let Some((previous_at, previous)) = &self.network_baseline {
                let elapsed_seconds = (collected_at - *previous_at).as_seconds_f64();
                delta::fill_network_rates(value, previous, elapsed_seconds);
            }
            self.network_baseline = Some((collected_at, value.clone()));
        }

        SystemSnapshot {
            collected_at,
            host: probe_section(&HostCollector, &self.paths),
            cpu,
            memory: probe_section(&MemoryCollector, &self.paths),
            load: probe_section(&LoadCollector, &self.paths),
            uptime: probe_section(&UptimeCollector, &self.paths),
            disks: probe_section(&disks, &self.paths),
            network,
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
    use std::path::{Path, PathBuf};

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

    fn available_cpu(snapshot: &SystemSnapshot) -> &CpuMetrics {
        match &snapshot.cpu {
            Section::Available { value } => value,
            Section::Unavailable { reason } => panic!("cpu unavailable: {reason}"),
        }
    }

    fn available_network(snapshot: &SystemSnapshot) -> &NetworkMetrics {
        match &snapshot.network {
            Section::Available { value } => value,
            Section::Unavailable { reason } => panic!("network unavailable: {reason}"),
        }
    }

    #[test]
    fn collects_every_section_from_fixtures() {
        let mut builder = SnapshotBuilder::new(fixture_paths(), Arc::new(FixedStats));
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
        let mut builder = SnapshotBuilder::new(paths, Arc::new(FixedStats));
        let snapshot = builder.collect(sample_time());

        assert!(matches!(snapshot.cpu, Section::Unavailable { .. }));
        assert!(matches!(snapshot.memory, Section::Unavailable { .. }));
        assert!(matches!(snapshot.host, Section::Available { .. }));
    }

    #[test]
    fn interval_metrics_are_absent_on_the_first_sample() {
        let mut builder = SnapshotBuilder::new(fixture_paths(), Arc::new(FixedStats));
        let snapshot = builder.collect(sample_time());

        assert_eq!(available_cpu(&snapshot).interval_usage_percent, None);
        assert_eq!(
            available_network(&snapshot).interfaces[0].rx_bytes_per_second,
            None
        );
    }

    #[test]
    fn interval_metrics_are_derived_between_samples() {
        const STAT_FIRST: &str = "cpu 100 0 50 800 50 0 0 0 0 0\n";
        const STAT_SECOND: &str = "cpu 200 0 100 1600 100 0 0 0 0 0\n";
        const NET_FIRST: &str = "  eth0: 1000 10 0 0 0 0 0 0 500 5 0 0 0 0 0 0\n";
        const NET_SECOND: &str = "  eth0: 3000 30 0 0 0 0 0 0 2500 25 0 0 0 0 0 0\n";

        let root = temp_root("interval");
        std::fs::write(root.join("proc/stat"), STAT_FIRST).expect("writes stat");
        std::fs::write(root.join("proc/net/dev"), NET_FIRST).expect("writes net dev");

        let paths = SystemPaths::new(root.join("proc"), root.join("sys"), root.clone());
        let mut builder = SnapshotBuilder::new(paths, Arc::new(FixedStats));

        let first = builder.collect(sample_time());
        assert_eq!(available_cpu(&first).interval_usage_percent, None);

        std::fs::write(root.join("proc/stat"), STAT_SECOND).expect("rewrites stat");
        std::fs::write(root.join("proc/net/dev"), NET_SECOND).expect("rewrites net dev");

        let second = builder
            .collect(OffsetDateTime::from_unix_timestamp(1_700_000_001).expect("valid timestamp"));
        assert_eq!(available_cpu(&second).interval_usage_percent, Some(15.0));

        let interface = &available_network(&second).interfaces[0];
        assert_eq!(interface.rx_bytes_per_second, Some(2000.0));
        assert_eq!(interface.tx_bytes_per_second, Some(2000.0));

        let _ = std::fs::remove_dir_all(&root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("aetherd-builder-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("proc/net")).expect("creates temp proc directory");
        root
    }
}
