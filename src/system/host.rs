use std::path::Path;

use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::{CollectorError, SystemCollector, SystemPaths};

/// Best-effort host information.
///
/// Every field is optional because a host may lack `/etc/os-release`, a
/// readable hostname, or a `btime` line. Missing metadata never fails the
/// endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct HostMetrics {
    /// Hostname reported by the kernel.
    pub hostname: Option<String>,
    /// Build architecture of this daemon binary.
    pub architecture: String,
    /// Kernel release, for example `6.8.0-45-generic`.
    pub kernel_release: Option<String>,
    /// Kernel build version string.
    pub kernel_version: Option<String>,
    /// Parsed `/etc/os-release`, when present.
    pub os: Option<OsRelease>,
    /// System boot time, when reported.
    pub boot_time: Option<OffsetDateTime>,
}

/// Selected fields from `/etc/os-release`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct OsRelease {
    /// Distribution identifier, for example `ubuntu`.
    pub id: Option<String>,
    /// Distribution name.
    pub name: Option<String>,
    /// Distribution version.
    pub version: Option<String>,
    /// Human-readable distribution description.
    pub pretty_name: Option<String>,
}

/// Collects host metadata, tolerating any missing source.
#[derive(Debug)]
pub(crate) struct HostCollector;

impl SystemCollector for HostCollector {
    type Metric = HostMetrics;

    fn name(&self) -> &'static str {
        "host"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let os = read_optional(&paths.host_path("etc/os-release"))
            .map(|content| parse_os_release(&content));
        let boot_time =
            read_optional(&paths.proc_file("stat")).and_then(|content| parse_boot_time(&content));

        Ok(HostMetrics {
            hostname: read_optional(&paths.proc_file("sys/kernel/hostname"))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            architecture: std::env::consts::ARCH.to_owned(),
            kernel_release: read_optional(&paths.proc_file("sys/kernel/osrelease"))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            kernel_version: read_optional(&paths.proc_file("sys/kernel/version"))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            os,
            boot_time,
        })
    }
}

fn read_optional(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

pub(crate) fn parse_os_release(input: &str) -> OsRelease {
    let field = |key: &str| {
        input.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| unquote(value.trim()).to_owned())
        })
    };

    OsRelease {
        id: field("ID"),
        name: field("NAME"),
        version: field("VERSION"),
        pretty_name: field("PRETTY_NAME"),
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value)
}

pub(crate) fn parse_boot_time(input: &str) -> Option<OffsetDateTime> {
    let seconds = input
        .lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .trim()
        .parse::<i64>()
        .ok()?;
    OffsetDateTime::from_unix_timestamp(seconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_and_unquoted_os_release_fields() {
        let input = "\
NAME=\"Ubuntu\"
ID=ubuntu
VERSION=\"24.04.1 LTS (Noble Numbat)\"
PRETTY_NAME=\"Ubuntu 24.04.1 LTS\"
";
        let release = parse_os_release(input);

        assert_eq!(release.id.as_deref(), Some("ubuntu"));
        assert_eq!(release.name.as_deref(), Some("Ubuntu"));
        assert_eq!(
            release.version.as_deref(),
            Some("24.04.1 LTS (Noble Numbat)")
        );
        assert_eq!(release.pretty_name.as_deref(), Some("Ubuntu 24.04.1 LTS"));
    }

    #[test]
    fn missing_os_release_fields_are_none() {
        let release = parse_os_release("ID=alpine\n");
        assert_eq!(release.id.as_deref(), Some("alpine"));
        assert_eq!(release.name, None);
        assert_eq!(release.version, None);
    }

    #[test]
    fn parses_boot_time_from_stat() {
        let input = "cpu 1 2 3 4\nbtime 1700000000\n";
        let boot = parse_boot_time(input).expect("btime present");
        assert_eq!(boot.unix_timestamp(), 1_700_000_000);
    }

    #[test]
    fn missing_boot_time_is_none() {
        assert_eq!(parse_boot_time("cpu 1 2 3 4\n"), None);
    }

    #[test]
    fn malformed_boot_time_is_none() {
        assert_eq!(parse_boot_time("btime not-a-number\n"), None);
    }
}
