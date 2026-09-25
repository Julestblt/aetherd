use serde::Serialize;
use utoipa::ToSchema;

use super::{CollectorError, ParseError, SystemCollector, SystemPaths, read_file};

/// CPU times read from `/proc/stat`.
///
/// Values are clock ticks in `USER_HZ` (conventionally 100 per second). Guest
/// time is already included in user and nice time and is not added again when
/// computing totals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct CpuTimes {
    /// Ticks spent in user mode.
    pub user_ticks: u64,
    /// Ticks spent in user mode with low priority.
    pub nice_ticks: u64,
    /// Ticks spent in system mode.
    pub system_ticks: u64,
    /// Ticks spent idle.
    pub idle_ticks: u64,
    /// Ticks waiting for I/O to complete.
    pub iowait_ticks: u64,
    /// Ticks servicing hardware interrupts.
    pub irq_ticks: u64,
    /// Ticks servicing software interrupts.
    pub softirq_ticks: u64,
    /// Ticks stolen by other virtual machines.
    pub steal_ticks: u64,
    /// Ticks spent running a virtual CPU for a guest.
    pub guest_ticks: u64,
    /// Ticks spent running a low-priority virtual CPU for a guest.
    pub guest_nice_ticks: u64,
}

/// Aggregate and per-core CPU metrics.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub(crate) struct CpuMetrics {
    /// Average CPU utilization since boot, as a percentage in `0..=100`.
    pub usage_percent: f64,
    /// CPU utilization over the interval since the previous sample, as a
    /// percentage in `0..=100`. `null` on the first sample and after a counter
    /// reset.
    pub interval_usage_percent: Option<f64>,
    /// Aggregate CPU times across all cores.
    pub total: CpuTimes,
    /// One entry per logical CPU.
    pub cores: Vec<CpuCore>,
}

/// CPU times and utilization for one logical CPU.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub(crate) struct CpuCore {
    /// Logical CPU identifier.
    pub id: u32,
    /// Average utilization since boot, as a percentage in `0..=100`.
    pub usage_percent: f64,
    /// Utilization over the interval since the previous sample. `null` on the
    /// first sample, after a counter reset, and for a core seen for the first
    /// time.
    pub interval_usage_percent: Option<f64>,
    /// CPU times for this core.
    pub times: CpuTimes,
}

impl CpuTimes {
    /// Total ticks, excluding guest time which is already counted in user and
    /// nice time.
    pub(crate) fn total_ticks(&self) -> u64 {
        self.user_ticks
            + self.nice_ticks
            + self.system_ticks
            + self.idle_ticks
            + self.iowait_ticks
            + self.irq_ticks
            + self.softirq_ticks
            + self.steal_ticks
    }

    /// Idle ticks, including time waiting for I/O.
    pub(crate) fn idle_total_ticks(&self) -> u64 {
        self.idle_ticks + self.iowait_ticks
    }
}

/// Collects CPU metrics from `/proc/stat`.
#[derive(Debug)]
pub(crate) struct CpuCollector;

impl SystemCollector for CpuCollector {
    type Metric = CpuMetrics;

    fn name(&self) -> &'static str {
        "cpu"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let path = paths.proc_file("stat");
        let content = read_file(path.clone())?;
        parse_cpu_stat(&content).map_err(|source| CollectorError::Parse { path, source })
    }
}

pub(crate) fn parse_cpu_stat(input: &str) -> Result<CpuMetrics, ParseError> {
    if input.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    let mut aggregate = None;
    let mut cores = Vec::new();

    for line in input.lines() {
        let mut tokens = line.split_whitespace();
        let Some(label) = tokens.next() else {
            continue;
        };
        let Some(suffix) = label.strip_prefix("cpu") else {
            continue;
        };
        let values: Vec<&str> = tokens.collect();

        if suffix.is_empty() {
            aggregate = Some(parse_cpu_times(&values)?);
        } else if let Ok(id) = suffix.parse::<u32>() {
            let times = parse_cpu_times(&values)?;
            cores.push(CpuCore {
                id,
                usage_percent: usage_percent(&times),
                interval_usage_percent: None,
                times,
            });
        }
    }

    let total = aggregate.ok_or_else(|| ParseError::MissingField("cpu".to_owned()))?;

    Ok(CpuMetrics {
        usage_percent: usage_percent(&total),
        interval_usage_percent: None,
        total,
        cores,
    })
}

fn parse_cpu_times(values: &[&str]) -> Result<CpuTimes, ParseError> {
    if values.len() < 4 {
        return Err(ParseError::MissingField("cpu times".to_owned()));
    }

    let read = |index: usize, field: &str| -> Result<u64, ParseError> {
        match values.get(index) {
            Some(raw) => raw.parse::<u64>().map_err(|_| ParseError::InvalidValue {
                field: field.to_owned(),
                value: (*raw).to_owned(),
            }),
            None => Ok(0),
        }
    };

    Ok(CpuTimes {
        user_ticks: read(0, "user")?,
        nice_ticks: read(1, "nice")?,
        system_ticks: read(2, "system")?,
        idle_ticks: read(3, "idle")?,
        iowait_ticks: read(4, "iowait")?,
        irq_ticks: read(5, "irq")?,
        softirq_ticks: read(6, "softirq")?,
        steal_ticks: read(7, "steal")?,
        guest_ticks: read(8, "guest")?,
        guest_nice_ticks: read(9, "guest_nice")?,
    })
}

fn usage_percent(times: &CpuTimes) -> f64 {
    let idle = times.idle_ticks + times.iowait_ticks;
    let total = times.user_ticks
        + times.nice_ticks
        + times.system_ticks
        + times.irq_ticks
        + times.softirq_ticks
        + times.steal_ticks
        + idle;

    if total == 0 {
        0.0
    } else {
        (total - idle) as f64 / total as f64 * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
cpu  100 0 50 850 0 0 0 0 0 0
cpu0 60 0 30 410 0 0 0 0 0 0
cpu1 40 0 20 440 0 0 0 0 0 0
intr 12345
ctxt 67890
btime 1700000000
processes 42
procs_running 1
procs_blocked 0
";

    #[test]
    fn parses_aggregate_and_cores() {
        let metrics = parse_cpu_stat(SAMPLE).expect("sample parses");

        assert_eq!(metrics.total.user_ticks, 100);
        assert_eq!(metrics.total.idle_ticks, 850);
        assert_eq!(metrics.cores.len(), 2);
        assert_eq!(metrics.cores[0].id, 0);
        assert_eq!(metrics.cores[1].id, 1);
        assert!((metrics.usage_percent - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trailing_fields_default_to_zero() {
        let input = "cpu 10 5 5 80\n";
        let metrics = parse_cpu_stat(input).expect("short line parses");
        assert_eq!(metrics.total.iowait_ticks, 0);
        assert_eq!(metrics.total.guest_ticks, 0);
    }

    #[test]
    fn missing_aggregate_line_is_an_error() {
        let input = "cpu0 1 2 3 4\n";
        let error = parse_cpu_stat(input).expect_err("aggregate required");
        assert_eq!(error, ParseError::MissingField("cpu".to_owned()));
    }

    #[test]
    fn malformed_value_is_an_error() {
        let input = "cpu 1 x 3 4\n";
        let error = parse_cpu_stat(input).expect_err("value must be numeric");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn too_few_fields_is_an_error() {
        let input = "cpu 1 2 3\n";
        let error = parse_cpu_stat(input).expect_err("four fields required");
        assert!(matches!(error, ParseError::MissingField(_)));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(
            parse_cpu_stat("   \n").expect_err("empty"),
            ParseError::Empty
        );
    }
}
