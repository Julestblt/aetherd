use std::path::PathBuf;

use crate::config::PathsConfig;

/// Filesystem roots for telemetry collection.
///
/// Paths are resolved relative to these roots so the daemon can run on a host
/// or read read-only host mounts from a container without code changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemPaths {
    /// Root of the procfs mount.
    pub proc: PathBuf,
    /// Root of the sysfs mount.
    pub sys: PathBuf,
    /// Root of the host filesystem.
    pub host_root: PathBuf,
}

impl SystemPaths {
    /// Creates a set of roots from explicit paths.
    #[must_use]
    pub fn new(
        proc: impl Into<PathBuf>,
        sys: impl Into<PathBuf>,
        host_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            proc: proc.into(),
            sys: sys.into(),
            host_root: host_root.into(),
        }
    }

    /// Resolves a file below the procfs root.
    #[must_use]
    pub fn proc_file(&self, name: &str) -> PathBuf {
        self.proc.join(name.trim_start_matches('/'))
    }

    /// Resolves a path below the sysfs root.
    #[must_use]
    pub fn sys_path(&self, relative: &str) -> PathBuf {
        self.sys.join(relative.trim_start_matches('/'))
    }

    /// Resolves a host-absolute path below the host root.
    #[must_use]
    pub fn host_path(&self, absolute: &str) -> PathBuf {
        self.host_root.join(absolute.trim_start_matches('/'))
    }
}

impl Default for SystemPaths {
    fn default() -> Self {
        Self {
            proc: PathBuf::from("/proc"),
            sys: PathBuf::from("/sys"),
            host_root: PathBuf::from("/"),
        }
    }
}

impl From<PathsConfig> for SystemPaths {
    fn from(value: PathsConfig) -> Self {
        Self {
            proc: value.proc,
            sys: value.sys,
            host_root: value.host_root,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_paths_under_each_root() {
        let paths = SystemPaths::new("/host/proc", "/host/sys", "/host/root");

        assert_eq!(paths.proc_file("stat"), PathBuf::from("/host/proc/stat"));
        assert_eq!(
            paths.proc_file("/meminfo"),
            PathBuf::from("/host/proc/meminfo")
        );
        assert_eq!(
            paths.sys_path("class/thermal"),
            PathBuf::from("/host/sys/class/thermal")
        );
        assert_eq!(
            paths.host_path("/etc/os-release"),
            PathBuf::from("/host/root/etc/os-release")
        );
    }

    #[test]
    fn default_roots_target_the_host() {
        let paths = SystemPaths::default();
        assert_eq!(paths.proc, PathBuf::from("/proc"));
        assert_eq!(paths.sys, PathBuf::from("/sys"));
        assert_eq!(paths.host_root, PathBuf::from("/"));
    }
}
