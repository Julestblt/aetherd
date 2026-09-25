use serde::Serialize;
use utoipa::ToSchema;

use super::{CollectorError, ParseError, SystemCollector, SystemPaths, read_file};

/// System uptime and idle time from `/proc/uptime`.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub(crate) struct UptimeMetrics {
    /// Seconds since the system booted.
    pub uptime_seconds: f64,
    /// Aggregate seconds the CPUs have been idle since boot.
    pub idle_seconds: f64,
}

/// Collects uptime from `/proc/uptime`.
#[derive(Debug)]
pub(crate) struct UptimeCollector;

impl SystemCollector for UptimeCollector {
    type Metric = UptimeMetrics;

    fn name(&self) -> &'static str {
        "uptime"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let path = paths.proc_file("uptime");
        let content = read_file(path.clone())?;
        parse_uptime(&content).map_err(|source| CollectorError::Parse { path, source })
    }
}

pub(crate) fn parse_uptime(input: &str) -> Result<UptimeMetrics, ParseError> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let Some(first) = tokens.first() else {
        return Err(ParseError::Empty);
    };

    let uptime_seconds = first.parse::<f64>().map_err(|_| ParseError::InvalidValue {
        field: "uptime_seconds".to_owned(),
        value: (*first).to_owned(),
    })?;

    let idle_seconds = match tokens.get(1) {
        Some(raw) => raw.parse::<f64>().map_err(|_| ParseError::InvalidValue {
            field: "idle_seconds".to_owned(),
            value: (*raw).to_owned(),
        })?,
        None => 0.0,
    };

    Ok(UptimeMetrics {
        uptime_seconds,
        idle_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uptime_and_idle() {
        let metrics = parse_uptime("12345.67 89012.34\n").expect("sample parses");
        assert!((metrics.uptime_seconds - 12345.67).abs() < f64::EPSILON);
        assert!((metrics.idle_seconds - 89012.34).abs() < f64::EPSILON);
    }

    #[test]
    fn missing_idle_defaults_to_zero() {
        let metrics = parse_uptime("42.5\n").expect("single value parses");
        assert!((metrics.uptime_seconds - 42.5).abs() < f64::EPSILON);
        assert!((metrics.idle_seconds).abs() < f64::EPSILON);
    }

    #[test]
    fn malformed_value_is_an_error() {
        let error = parse_uptime("soon\n").expect_err("number required");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_uptime("  \n").expect_err("empty"), ParseError::Empty);
    }
}
