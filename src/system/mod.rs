//! Linux telemetry domain: typed metrics and `/proc`, `/sys` parsers.

pub(crate) mod cpu;
pub(crate) mod disks;
pub(crate) mod host;
pub(crate) mod load;
pub(crate) mod memory;
pub(crate) mod network;
mod paths;
pub(crate) mod uptime;

use std::path::PathBuf;

pub(crate) use disks::RealMountStats;
pub use disks::{DiskUsage, MountStats};
pub use paths::SystemPaths;

/// Errors produced while collecting a metric.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CollectorError {
    /// The metric source could not be read.
    #[error("cannot read {}", .path.display())]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The metric source was read but could not be parsed.
    #[error("cannot parse {}", .path.display())]
    Parse {
        /// The file that could not be parsed.
        path: PathBuf,
        /// The underlying parse error.
        #[source]
        source: ParseError,
    },
}

/// Errors produced while parsing a metric source.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum ParseError {
    /// The input had no usable content.
    #[error("input is empty")]
    Empty,
    /// A required field was absent.
    #[error("missing field: {0}")]
    MissingField(String),
    /// A field had a value that could not be interpreted.
    #[error("invalid value for {field}: {value}")]
    InvalidValue {
        /// The field that failed to parse.
        field: String,
        /// The offending value.
        value: String,
    },
}

/// A source of one system metric.
///
/// Implementations read through [`SystemPaths`], so the same collector works on
/// a host, inside a container, and against test fixtures.
pub(crate) trait SystemCollector {
    /// The metric produced by this collector.
    type Metric;

    /// A short, stable name used in diagnostics.
    fn name(&self) -> &'static str;

    /// Collects the metric, or explains why it could not be collected.
    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError>;
}

pub(crate) fn read_file(path: PathBuf) -> Result<String, CollectorError> {
    std::fs::read_to_string(&path).map_err(|source| CollectorError::Read { path, source })
}
