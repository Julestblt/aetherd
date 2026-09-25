use std::io;
use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use utoipa::ToSchema;

use super::{CollectorError, ParseError, SystemCollector, SystemPaths, read_file};

/// Filesystem types excluded from the default disk response.
///
/// These are kernel-internal or synthetic filesystems such as `proc`, `sysfs`,
/// `cgroup2`, and container `overlay` layers. They are collected internally but
/// omitted from the API to keep the response focused on storage a user can act
/// on.
pub(crate) const PSEUDO_FS_TYPES: &[&str] = &[
    "autofs",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "overlay",
    "proc",
    "pstore",
    "ramfs",
    "securityfs",
    "sysfs",
    "tmpfs",
    "tracefs",
];

/// One mount described by `/proc/mounts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MountEntry {
    pub(crate) source: String,
    pub(crate) mount_point: String,
    pub(crate) fs_type: String,
}

impl MountEntry {
    fn is_pseudo(&self) -> bool {
        PSEUDO_FS_TYPES.contains(&self.fs_type.as_str())
    }
}

/// Capacity statistics for one mounted filesystem.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct DiskUsage {
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Free bytes available to the superuser.
    pub free_bytes: u64,
    /// Free bytes available to unprivileged users.
    pub available_bytes: u64,
    /// Used bytes: `total - free`.
    pub used_bytes: u64,
    /// Used space as a percentage of addressable space, in `0..=100`.
    pub used_percent: f64,
}

/// A mounted filesystem with optional capacity statistics.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub(crate) struct FilesystemMetrics {
    /// Source device or pseudo-source.
    pub source: String,
    /// Mount point.
    pub mount_point: String,
    /// Filesystem type.
    pub fs_type: String,
    /// Capacity statistics, absent when they could not be read.
    pub usage: Option<DiskUsage>,
}

/// Filesystem metrics for all included mounts.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub(crate) struct DisksMetrics {
    /// Mounted, non-pseudo filesystems.
    pub filesystems: Vec<FilesystemMetrics>,
}

/// Reads capacity statistics for a mount point.
pub trait MountStats: std::fmt::Debug + Send + Sync {
    /// Returns capacity statistics for `path`, or the underlying I/O error.
    fn usage(&self, path: &Path) -> Result<DiskUsage, io::Error>;
}

/// Reads capacity statistics through the `statvfs` syscall.
#[derive(Debug, Default)]
pub(crate) struct RealMountStats;

impl MountStats for RealMountStats {
    fn usage(&self, path: &Path) -> Result<DiskUsage, io::Error> {
        let stats = rustix::fs::statvfs(path)
            .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))?;

        let block_size = stats.f_frsize;
        let total = stats.f_blocks.saturating_mul(block_size);
        let free = stats.f_bfree.saturating_mul(block_size);
        let available = stats.f_bavail.saturating_mul(block_size);
        let used = total.saturating_sub(free);
        let addressable = used.saturating_add(available);

        Ok(DiskUsage {
            total_bytes: total,
            free_bytes: free,
            available_bytes: available,
            used_bytes: used,
            used_percent: if addressable == 0 {
                0.0
            } else {
                used as f64 / addressable as f64 * 100.0
            },
        })
    }
}

/// Collects filesystem metrics from `/proc/mounts` and [`MountStats`].
#[derive(Debug)]
pub(crate) struct DiskCollector {
    stats: Arc<dyn MountStats>,
}

impl DiskCollector {
    pub(crate) fn new(stats: Arc<dyn MountStats>) -> Self {
        Self { stats }
    }
}

impl SystemCollector for DiskCollector {
    type Metric = DisksMetrics;

    fn name(&self) -> &'static str {
        "disks"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let path = paths.proc_file("mounts");
        let content = read_file(path.clone())?;
        let mounts =
            parse_mounts(&content).map_err(|source| CollectorError::Parse { path, source })?;

        let filesystems = mounts
            .into_iter()
            .filter(|entry| !entry.is_pseudo())
            .map(|entry| {
                let host_path = paths.host_path(&entry.mount_point);
                let usage = match self.stats.usage(&host_path) {
                    Ok(usage) => Some(usage),
                    Err(error) => {
                        tracing::debug!(
                            mount_point = %entry.mount_point,
                            error = %error,
                            "filesystem usage unavailable"
                        );
                        None
                    }
                };

                FilesystemMetrics {
                    source: entry.source,
                    mount_point: entry.mount_point,
                    fs_type: entry.fs_type,
                    usage,
                }
            })
            .collect();

        Ok(DisksMetrics { filesystems })
    }
}

pub(crate) fn parse_mounts(input: &str) -> Result<Vec<MountEntry>, ParseError> {
    let mut entries = Vec::new();

    for line in input.lines() {
        let mut fields = line.split_whitespace();
        let Some(source) = fields.next() else {
            continue;
        };
        let Some(mount_point) = fields.next() else {
            return Err(ParseError::InvalidValue {
                field: "mounts".to_owned(),
                value: line.to_owned(),
            });
        };
        let Some(fs_type) = fields.next() else {
            return Err(ParseError::InvalidValue {
                field: "mounts".to_owned(),
                value: line.to_owned(),
            });
        };
        let Some(_options) = fields.next() else {
            return Err(ParseError::InvalidValue {
                field: "mounts".to_owned(),
                value: line.to_owned(),
            });
        };

        entries.push(MountEntry {
            source: unescape_octal(source),
            mount_point: unescape_octal(mount_point),
            fs_type: fs_type.to_owned(),
        });
    }

    if entries.is_empty() {
        return Err(ParseError::Empty);
    }

    Ok(entries)
}

fn unescape_octal(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }

        let mut octal = String::new();
        while octal.len() < 3 {
            match chars.peek() {
                Some(digit @ '0'..='7') => {
                    octal.push(*digit);
                    chars.next();
                }
                _ => break,
            }
        }

        match u32::from_str_radix(&octal, 8).ok().and_then(char::from_u32) {
            Some(decoded) if octal.len() == 3 => output.push(decoded),
            _ => {
                output.push('\\');
                output.push_str(&octal);
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
/dev/sda1 / ext4 rw,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
tmpfs /run tmpfs rw,nosuid,nodev 0 0
/dev/sdb1 /data\\040volume ext4 rw,relatime 0 0
";

    #[test]
    fn parses_all_mounts_and_unescapes_paths() {
        let mounts = parse_mounts(SAMPLE).expect("sample parses");

        assert_eq!(mounts.len(), 5);
        assert_eq!(mounts[0].source, "/dev/sda1");
        assert_eq!(mounts[0].mount_point, "/");
        assert_eq!(mounts[0].fs_type, "ext4");
        assert_eq!(mounts[4].mount_point, "/data volume");
    }

    #[test]
    fn pseudo_filesystems_are_identified() {
        let mounts = parse_mounts(SAMPLE).expect("sample parses");
        let included: Vec<&str> = mounts
            .iter()
            .filter(|entry| !entry.is_pseudo())
            .map(|entry| entry.mount_point.as_str())
            .collect();

        assert_eq!(included, vec!["/", "/data volume"]);
    }

    #[test]
    fn unescapes_common_octal_sequences() {
        assert_eq!(unescape_octal("/data\\040volume"), "/data volume");
        assert_eq!(unescape_octal("a\\011b"), "a\tb");
        assert_eq!(unescape_octal("a\\134b"), "a\\b");
        assert_eq!(unescape_octal("plain"), "plain");
    }

    #[test]
    fn truncated_line_is_an_error() {
        let error = parse_mounts("/dev/sda1 / ext4\n").expect_err("four fields required");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_mounts("").expect_err("empty"), ParseError::Empty);
    }
}
