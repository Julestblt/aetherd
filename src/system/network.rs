use serde::Serialize;
use utoipa::ToSchema;

use super::{CollectorError, ParseError, SystemCollector, SystemPaths, read_file};

/// An interface entry from `/proc/net/dev`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct NetworkInterface {
    /// Interface name.
    pub name: String,
    /// Bytes received since boot.
    pub rx_bytes: u64,
    /// Packets received since boot.
    pub rx_packets: u64,
    /// Receive errors since boot.
    pub rx_errors: u64,
    /// Received packets dropped since boot.
    pub rx_dropped: u64,
    /// Bytes transmitted since boot.
    pub tx_bytes: u64,
    /// Packets transmitted since boot.
    pub tx_packets: u64,
    /// Transmit errors since boot.
    pub tx_errors: u64,
    /// Transmitted packets dropped since boot.
    pub tx_dropped: u64,
}

/// Per-interface network counters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct NetworkMetrics {
    /// One entry per network interface.
    pub interfaces: Vec<NetworkInterface>,
}

/// Collects interface counters from `/proc/net/dev`.
#[derive(Debug)]
pub(crate) struct NetworkCollector;

impl SystemCollector for NetworkCollector {
    type Metric = NetworkMetrics;

    fn name(&self) -> &'static str {
        "network"
    }

    fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
        let path = paths.proc_file("net/dev");
        let content = read_file(path.clone())?;
        parse_net_dev(&content).map_err(|source| CollectorError::Parse { path, source })
    }
}

pub(crate) fn parse_net_dev(input: &str) -> Result<NetworkMetrics, ParseError> {
    let mut interfaces = Vec::new();

    for line in input.lines() {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let values: Vec<&str> = counters.split_whitespace().collect();
        if values.len() < 9 {
            continue;
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

        interfaces.push(NetworkInterface {
            name: name.trim().to_owned(),
            rx_bytes: read(0, "rx_bytes")?,
            rx_packets: read(1, "rx_packets")?,
            rx_errors: read(2, "rx_errors")?,
            rx_dropped: read(3, "rx_dropped")?,
            tx_bytes: read(8, "tx_bytes")?,
            tx_packets: read(9, "tx_packets")?,
            tx_errors: read(10, "tx_errors")?,
            tx_dropped: read(11, "tx_dropped")?,
        });
    }

    if interfaces.is_empty() {
        return Err(ParseError::MissingField("interfaces".to_owned()));
    }

    Ok(NetworkMetrics { interfaces })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1000      10    0    0    0     0          0         0     1000      10    0    0    0     0       0          0
  eth0: 5000      50    1    2    0     0          0         0     6000      60    3    4    0     0       0          0
";

    #[test]
    fn parses_interfaces_and_counters() {
        let metrics = parse_net_dev(SAMPLE).expect("sample parses");

        assert_eq!(metrics.interfaces.len(), 2);
        let eth0 = &metrics.interfaces[1];
        assert_eq!(eth0.name, "eth0");
        assert_eq!(eth0.rx_bytes, 5000);
        assert_eq!(eth0.rx_packets, 50);
        assert_eq!(eth0.rx_errors, 1);
        assert_eq!(eth0.rx_dropped, 2);
        assert_eq!(eth0.tx_bytes, 6000);
        assert_eq!(eth0.tx_packets, 60);
        assert_eq!(eth0.tx_errors, 3);
        assert_eq!(eth0.tx_dropped, 4);
    }

    #[test]
    fn header_lines_are_ignored() {
        let metrics = parse_net_dev(SAMPLE).expect("headers skipped");
        assert!(metrics.interfaces.iter().all(|iface| iface.name != "face"));
    }

    #[test]
    fn short_counter_lines_are_skipped() {
        let input = "  lo: 1 2 3\n  eth0: 1 2 3 4 0 0 0 0 5 6 7 8\n";
        let metrics = parse_net_dev(input).expect("one valid interface");
        assert_eq!(metrics.interfaces.len(), 1);
        assert_eq!(metrics.interfaces[0].name, "eth0");
    }

    #[test]
    fn malformed_counter_is_an_error() {
        let input = "  eth0: x 2 3 4 0 0 0 0 5 6 7 8\n";
        let error = parse_net_dev(input).expect_err("numeric counters required");
        assert!(matches!(error, ParseError::InvalidValue { .. }));
    }

    #[test]
    fn no_interfaces_is_an_error() {
        let error = parse_net_dev("just a header\n").expect_err("at least one interface");
        assert!(matches!(error, ParseError::MissingField(_)));
    }
}
