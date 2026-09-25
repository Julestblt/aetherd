use std::collections::HashMap;

use serde::Serialize;
use utoipa::ToSchema;

use super::{CollectorError, ParseError, SystemCollector, SystemPaths, read_file};

/// Memory and swap usage read from `/proc/meminfo`, in bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct MemoryMetrics {
    /// Total usable RAM.
    pub total_bytes: u64,
    /// Unused RAM.
    pub free_bytes: u64,
    /// RAM available to new workloads without swapping, when reported.
    pub available_bytes: Option<u64>,
    /// Used RAM: `total - available` when `MemAvailable` is reported, otherwise
    /// `total - free - buffers - cached`.
    pub used_bytes: u64,
    /// Temporary storage for raw block device data.
    pub buffers_bytes: u64,
    /// Page cache memory.
    pub cached_bytes: u64,
    /// Total swap space.
    pub swap_total_bytes: u64,
    /// Unused swap space.
    pub swap_free_bytes: u64,
    /// Used swap space.
    pub swap_used_bytes: u64,
}

/// Collects memory metrics from `/proc/meminfo`.
#[derive(Debug)]
pub(crate) struct MemoryCollector;

impl SystemCollector for MemoryCollector {
    type Metric = MemoryMetrics;

    fn name(&self) -> &'static str {
        "memory"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let path = paths.proc_file("meminfo");
        let content = read_file(path.clone())?;
        parse_meminfo(&content).map_err(|source| CollectorError::Parse { path, source })
    }
}

pub(crate) fn parse_meminfo(input: &str) -> Result<MemoryMetrics, ParseError> {
    if input.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    let mut fields: HashMap<&str, u64> = HashMap::new();
    for line in input.lines() {
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let Some(value) = parts.next() else {
            continue;
        };
        let key = key.strip_suffix(':').unwrap_or(key);
        let kibibytes = value.parse::<u64>().map_err(|_| ParseError::InvalidValue {
            field: key.to_owned(),
            value: value.to_owned(),
        })?;
        fields.insert(key, kibibytes.saturating_mul(1024));
    }

    let total = required(&fields, "MemTotal")?;
    let free = required(&fields, "MemFree")?;
    let available = fields.get("MemAvailable").copied();
    let buffers = fields.get("Buffers").copied().unwrap_or(0);
    let cached = fields.get("Cached").copied().unwrap_or(0);
    let swap_total = fields.get("SwapTotal").copied().unwrap_or(0);
    let swap_free = fields.get("SwapFree").copied().unwrap_or(0);

    let used = match available {
        Some(available) => total.saturating_sub(available),
        None => total
            .saturating_sub(free)
            .saturating_sub(buffers)
            .saturating_sub(cached),
    };

    Ok(MemoryMetrics {
        total_bytes: total,
        free_bytes: free,
        available_bytes: available,
        used_bytes: used,
        buffers_bytes: buffers,
        cached_bytes: cached,
        swap_total_bytes: swap_total,
        swap_free_bytes: swap_free,
        swap_used_bytes: swap_total.saturating_sub(swap_free),
    })
}

fn required(fields: &HashMap<&str, u64>, key: &str) -> Result<u64, ParseError> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| ParseError::MissingField(key.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
MemTotal:       16384000 kB
MemFree:         8192000 kB
MemAvailable:   12000000 kB
Buffers:          100000 kB
Cached:          2000000 kB
SwapTotal:       4096000 kB
SwapFree:        4096000 kB
";

    #[test]
    fn parses_kibibytes_into_bytes() {
        let metrics = parse_meminfo(SAMPLE).expect("sample parses");

        assert_eq!(metrics.total_bytes, 16_384_000 * 1024);
        assert_eq!(metrics.free_bytes, 8_192_000 * 1024);
        assert_eq!(metrics.available_bytes, Some(12_000_000 * 1024));
        assert_eq!(metrics.used_bytes, (16_384_000 - 12_000_000) * 1024);
        assert_eq!(metrics.swap_used_bytes, 0);
    }

    #[test]
    fn missing_mem_available_falls_back_to_free_buffers_cached() {
        let input = "\
MemTotal: 1000 kB
MemFree: 400 kB
Buffers: 100 kB
Cached: 200 kB
";
        let metrics = parse_meminfo(input).expect("fallback parses");

        assert_eq!(metrics.available_bytes, None);
        assert_eq!(metrics.used_bytes, 300 * 1024);
    }

    #[test]
    fn missing_total_is_an_error() {
        let input = "MemFree: 1 kB\n";
        let error = parse_meminfo(input).expect_err("MemTotal required");
        assert_eq!(error, ParseError::MissingField("MemTotal".to_owned()));
    }

    #[test]
    fn malformed_value_is_an_error() {
        let input = "MemTotal: lots kB\n";
        let error = parse_meminfo(input).expect_err("value must be numeric");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn lines_without_values_are_ignored() {
        let input = "\
MemTotal: 1000 kB
HugePages_Total:
MemFree: 500 kB
";
        let metrics = parse_meminfo(input).expect("incomplete line skipped");
        assert_eq!(metrics.total_bytes, 1000 * 1024);
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_meminfo("\n").expect_err("empty"), ParseError::Empty);
    }
}
