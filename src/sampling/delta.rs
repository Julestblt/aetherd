//! Interval-derived metrics computed from consecutive samples.

use crate::system::cpu::{CpuMetrics, CpuTimes};
use crate::system::network::NetworkMetrics;

/// CPU utilization between two cumulative counter readings.
///
/// Returns `None` when no time has elapsed or when a counter went backwards,
/// which marks a reboot or counter reset.
pub(crate) fn cpu_interval_percent(previous: &CpuTimes, current: &CpuTimes) -> Option<f64> {
    let total_previous = previous.total_ticks();
    let total_current = current.total_ticks();
    let idle_previous = previous.idle_total_ticks();
    let idle_current = current.idle_total_ticks();

    if total_current < total_previous || idle_current < idle_previous {
        return None;
    }

    let total_delta = total_current - total_previous;
    if total_delta == 0 {
        return None;
    }

    let busy_delta = total_delta - (idle_current - idle_previous);
    Some(busy_delta as f64 / total_delta as f64 * 100.0)
}

/// Fills the interval utilization on every core that has a previous reading.
pub(crate) fn fill_cpu_interval(current: &mut CpuMetrics, previous: &CpuMetrics) {
    current.interval_usage_percent = cpu_interval_percent(&previous.total, &current.total);

    for core in &mut current.cores {
        core.interval_usage_percent = previous
            .cores
            .iter()
            .find(|previous_core| previous_core.id == core.id)
            .and_then(|previous_core| cpu_interval_percent(&previous_core.times, &core.times));
    }
}

/// Byte counter difference expressed as a rate per second.
///
/// Returns `None` for a non-positive interval or a counter that went backwards.
pub(crate) fn counter_rate(previous: u64, current: u64, elapsed_seconds: f64) -> Option<f64> {
    if elapsed_seconds <= 0.0 || current < previous {
        return None;
    }
    Some((current - previous) as f64 / elapsed_seconds)
}

/// Fills per-interface RX/TX rates for interfaces present in both samples.
pub(crate) fn fill_network_rates(
    current: &mut NetworkMetrics,
    previous: &NetworkMetrics,
    elapsed_seconds: f64,
) {
    for interface in &mut current.interfaces {
        let Some(previous_interface) = previous
            .interfaces
            .iter()
            .find(|candidate| candidate.name == interface.name)
        else {
            continue;
        };

        interface.rx_bytes_per_second = counter_rate(
            previous_interface.rx_bytes,
            interface.rx_bytes,
            elapsed_seconds,
        );
        interface.tx_bytes_per_second = counter_rate(
            previous_interface.tx_bytes,
            interface.tx_bytes,
            elapsed_seconds,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::cpu::CpuCore;
    use crate::system::network::NetworkInterface;

    fn times(user: u64, system: u64, idle: u64, iowait: u64) -> CpuTimes {
        CpuTimes {
            user_ticks: user,
            nice_ticks: 0,
            system_ticks: system,
            idle_ticks: idle,
            iowait_ticks: iowait,
            irq_ticks: 0,
            softirq_ticks: 0,
            steal_ticks: 0,
            guest_ticks: 0,
            guest_nice_ticks: 0,
        }
    }

    fn interface(name: &str, rx_bytes: u64, tx_bytes: u64) -> NetworkInterface {
        NetworkInterface {
            name: name.to_owned(),
            rx_bytes,
            rx_packets: 0,
            rx_errors: 0,
            rx_dropped: 0,
            tx_bytes,
            tx_packets: 0,
            tx_errors: 0,
            tx_dropped: 0,
            rx_bytes_per_second: None,
            tx_bytes_per_second: None,
        }
    }

    #[test]
    fn computes_interval_percentage_from_counter_delta() {
        let previous = times(100, 50, 800, 50);
        let current = times(200, 100, 1600, 100);

        assert_eq!(cpu_interval_percent(&previous, &current), Some(15.0));
    }

    #[test]
    fn identical_counters_have_no_interval_percentage() {
        let sample = times(100, 50, 800, 50);
        assert_eq!(cpu_interval_percent(&sample, &sample), None);
    }

    #[test]
    fn counter_reset_has_no_interval_percentage() {
        let previous = times(200, 100, 1600, 100);
        let current = times(100, 50, 800, 50);
        assert_eq!(cpu_interval_percent(&previous, &current), None);
    }

    #[test]
    fn fills_per_core_interval_and_skips_new_cores() {
        let previous = CpuMetrics {
            usage_percent: 0.0,
            interval_usage_percent: None,
            total: times(100, 50, 800, 50),
            cores: vec![CpuCore {
                id: 0,
                usage_percent: 0.0,
                interval_usage_percent: None,
                times: times(100, 50, 800, 50),
            }],
        };
        let mut current = CpuMetrics {
            usage_percent: 0.0,
            interval_usage_percent: None,
            total: times(200, 100, 1600, 100),
            cores: vec![
                CpuCore {
                    id: 0,
                    usage_percent: 0.0,
                    interval_usage_percent: None,
                    times: times(200, 100, 1600, 100),
                },
                CpuCore {
                    id: 1,
                    usage_percent: 0.0,
                    interval_usage_percent: None,
                    times: times(50, 25, 400, 25),
                },
            ],
        };

        fill_cpu_interval(&mut current, &previous);

        assert_eq!(current.interval_usage_percent, Some(15.0));
        assert_eq!(current.cores[0].interval_usage_percent, Some(15.0));
        assert_eq!(current.cores[1].interval_usage_percent, None);
    }

    #[test]
    fn computes_byte_rate_over_elapsed_seconds() {
        assert_eq!(counter_rate(1000, 3000, 2.0), Some(1000.0));
        assert_eq!(counter_rate(1000, 1000, 1.0), Some(0.0));
    }

    #[test]
    fn rate_is_none_on_reset_or_non_positive_interval() {
        assert_eq!(counter_rate(3000, 1000, 1.0), None);
        assert_eq!(counter_rate(1000, 3000, 0.0), None);
        assert_eq!(counter_rate(1000, 3000, -1.0), None);
    }

    #[test]
    fn network_rates_skip_an_interfaces_first_observation() {
        let previous = NetworkMetrics {
            interfaces: vec![interface("eth0", 1000, 500)],
        };
        let mut current = NetworkMetrics {
            interfaces: vec![interface("eth0", 3000, 2500), interface("eth1", 10, 20)],
        };

        fill_network_rates(&mut current, &previous, 1.0);

        assert_eq!(current.interfaces[0].rx_bytes_per_second, Some(2000.0));
        assert_eq!(current.interfaces[0].tx_bytes_per_second, Some(2000.0));
        assert_eq!(current.interfaces[1].rx_bytes_per_second, None);
        assert_eq!(current.interfaces[1].tx_bytes_per_second, None);
    }

    #[test]
    fn network_rates_are_none_on_counter_reset() {
        let previous = NetworkMetrics {
            interfaces: vec![interface("eth0", 5000, 5000)],
        };
        let mut current = NetworkMetrics {
            interfaces: vec![interface("eth0", 10, 20)],
        };

        fill_network_rates(&mut current, &previous, 1.0);

        assert_eq!(current.interfaces[0].rx_bytes_per_second, None);
        assert_eq!(current.interfaces[0].tx_bytes_per_second, None);
    }
}
