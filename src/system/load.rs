use serde::Serialize;
use utoipa::ToSchema;

use super::{CollectorError, ParseError, SystemCollector, SystemPaths, read_file};

/// System load average and process counts from `/proc/loadavg`.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub(crate) struct LoadMetrics {
    /// One-minute load average.
    pub load1: f64,
    /// Five-minute load average.
    pub load5: f64,
    /// Fifteen-minute load average.
    pub load15: f64,
    /// Currently runnable processes.
    pub runnable: u64,
    /// Total processes on the host.
    pub total_processes: u64,
    /// Most recently created process identifier.
    pub last_pid: u64,
}

/// Collects load metrics from `/proc/loadavg`.
#[derive(Debug)]
pub(crate) struct LoadCollector;

impl SystemCollector for LoadCollector {
    type Metric = LoadMetrics;

    fn name(&self) -> &'static str {
        "load"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let path = paths.proc_file("loadavg");
        let content = read_file(path.clone())?;
        parse_loadavg(&content).map_err(|source| CollectorError::Parse { path, source })
    }
}

pub(crate) fn parse_loadavg(input: &str) -> Result<LoadMetrics, ParseError> {
    let tokens: Vec<&str> = input.split_whitespace().collect();

    if tokens.is_empty() {
        return Err(ParseError::Empty);
    }
    if tokens.len() < 5 {
        return Err(ParseError::MissingField("loadavg".to_owned()));
    }

    let (runnable, total) = tokens[3]
        .split_once('/')
        .ok_or_else(|| ParseError::InvalidValue {
            field: "runnable".to_owned(),
            value: tokens[3].to_owned(),
        })?;

    Ok(LoadMetrics {
        load1: parse_f64(tokens[0], "load1")?,
        load5: parse_f64(tokens[1], "load5")?,
        load15: parse_f64(tokens[2], "load15")?,
        runnable: parse_u64(runnable, "runnable")?,
        total_processes: parse_u64(total, "total_processes")?,
        last_pid: parse_u64(tokens[4], "last_pid")?,
    })
}

fn parse_f64(raw: &str, field: &str) -> Result<f64, ParseError> {
    raw.parse::<f64>().map_err(|_| ParseError::InvalidValue {
        field: field.to_owned(),
        value: raw.to_owned(),
    })
}

fn parse_u64(raw: &str, field: &str) -> Result<u64, ParseError> {
    raw.parse::<u64>().map_err(|_| ParseError::InvalidValue {
        field: field.to_owned(),
        value: raw.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_load_average_and_counts() {
        let metrics = parse_loadavg("1.50 2.25 3.00 2/1234 56789\n").expect("sample parses");

        assert!((metrics.load1 - 1.5).abs() < f64::EPSILON);
        assert!((metrics.load5 - 2.25).abs() < f64::EPSILON);
        assert!((metrics.load15 - 3.0).abs() < f64::EPSILON);
        assert_eq!(metrics.runnable, 2);
        assert_eq!(metrics.total_processes, 1234);
        assert_eq!(metrics.last_pid, 56789);
    }

    #[test]
    fn too_few_fields_is_an_error() {
        let error = parse_loadavg("1.0 2.0\n").expect_err("five fields required");
        assert!(matches!(error, ParseError::MissingField(_)));
    }

    #[test]
    fn malformed_runnable_pair_is_an_error() {
        let error = parse_loadavg("1.0 2.0 3.0 nope 5\n").expect_err("pair required");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn malformed_float_is_an_error() {
        let error = parse_loadavg("x 2.0 3.0 1/2 5\n").expect_err("float required");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_loadavg("").expect_err("empty"), ParseError::Empty);
    }
}
