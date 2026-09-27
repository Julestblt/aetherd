use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;

/// Age in seconds beyond which a device is reported as offline.
pub(crate) const ONLINE_THRESHOLD_SECONDS: i64 = 120;

/// Tailnet machines observed through the Tailscale HTTP API.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct TailscaleSnapshot {
    /// RFC3339 UTC timestamp of the last successful refresh.
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    /// Tailnet the devices belong to.
    pub tailnet: String,
    /// Machines currently known to the tailnet.
    pub devices: Vec<TailscaleDevice>,
}

/// A single Tailscale machine.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct TailscaleDevice {
    /// Stable device identifier.
    pub id: String,
    /// Short hostname.
    pub hostname: String,
    /// Fully qualified `MagicDNS` name.
    pub dns_name: String,
    /// Operating system reported by the client.
    pub os: String,
    /// Tailnet IP addresses.
    pub addresses: Vec<String>,
    /// Whether the device was seen in the last 120 seconds.
    ///
    /// The Tailscale API exposes no online flag, so this is inferred from
    /// `last_seen`; an absent or unparseable timestamp is reported as offline.
    pub online: bool,
    /// RFC3339 UTC timestamp of the last handshake, when the API reported a
    /// parseable value.
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_seen: Option<OffsetDateTime>,
    /// Whether the device is authorized to join the tailnet.
    pub authorized: bool,
    /// ACL tags applied to the device.
    pub tags: Vec<String>,
}

/// Upstream payload of `GET /api/v2/tailnet/{tailnet}/devices`.
///
/// Unknown fields are ignored so a new upstream field never breaks parsing.
#[derive(Debug, Deserialize)]
pub(crate) struct DevicesResponse {
    pub(crate) devices: Vec<WireDevice>,
}

/// One upstream device entry, limited to the fields aetherd maps.
#[derive(Debug, Deserialize)]
pub(crate) struct WireDevice {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) hostname: String,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) os: String,
    #[serde(default)]
    pub(crate) addresses: Vec<String>,
    #[serde(default)]
    pub(crate) authorized: bool,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default, rename = "lastSeen")]
    pub(crate) last_seen: Option<String>,
}

impl DevicesResponse {
    pub(crate) fn into_snapshot(
        self,
        tailnet: &str,
        collected_at: OffsetDateTime,
    ) -> TailscaleSnapshot {
        TailscaleSnapshot {
            collected_at,
            tailnet: tailnet.to_owned(),
            devices: self
                .devices
                .into_iter()
                .map(|device| device.into_model(collected_at))
                .collect(),
        }
    }
}

impl WireDevice {
    fn into_model(self, collected_at: OffsetDateTime) -> TailscaleDevice {
        let last_seen = self.last_seen.as_deref().and_then(parse_last_seen);
        TailscaleDevice {
            id: self.id,
            hostname: self.hostname,
            dns_name: self.name,
            os: self.os,
            addresses: self.addresses,
            online: is_online(last_seen, collected_at),
            last_seen,
            authorized: self.authorized,
            tags: self.tags,
        }
    }
}

fn is_online(last_seen: Option<OffsetDateTime>, collected_at: OffsetDateTime) -> bool {
    last_seen.is_some_and(|seen| collected_at - seen < Duration::seconds(ONLINE_THRESHOLD_SECONDS))
}

fn parse_last_seen(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "devices": [
            {
                "id": "node-1",
                "hostname": "homelab",
                "name": "homelab.example.ts.net",
                "os": "linux",
                "addresses": ["100.64.0.1"],
                "authorized": true,
                "tags": ["tag:homelab"],
                "lastSeen": "2023-11-14T22:13:20Z",
                "unexpected": {"nested": true}
            }
        ]
    }"#;

    fn collected_at() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid timestamp")
    }

    fn parse(body: &str) -> Result<TailscaleSnapshot, serde_json::Error> {
        serde_json::from_str::<DevicesResponse>(body)
            .map(|response| response.into_snapshot("example.ts.net", collected_at()))
    }

    #[test]
    fn parses_devices_into_the_public_model() {
        let snapshot = parse(SAMPLE).expect("payload parses");

        assert_eq!(snapshot.tailnet, "example.ts.net");
        assert_eq!(snapshot.collected_at, collected_at());
        assert_eq!(
            snapshot.devices,
            vec![TailscaleDevice {
                id: "node-1".to_owned(),
                hostname: "homelab".to_owned(),
                dns_name: "homelab.example.ts.net".to_owned(),
                os: "linux".to_owned(),
                addresses: vec!["100.64.0.1".to_owned()],
                online: true,
                last_seen: Some(collected_at()),
                authorized: true,
                tags: vec!["tag:homelab".to_owned()],
            }]
        );
    }

    #[test]
    fn empty_device_list_is_valid() {
        let snapshot = parse(r#"{"devices":[]}"#).expect("payload parses");

        assert!(snapshot.devices.is_empty());
    }

    #[test]
    fn missing_devices_field_is_rejected() {
        assert!(parse(r#"{"unexpected":true}"#).is_err());
    }

    #[test]
    fn missing_device_id_is_rejected() {
        assert!(parse(r#"{"devices":[{"hostname":"homelab"}]}"#).is_err());
    }

    #[test]
    fn malformed_json_is_rejected() {
        assert!(parse("{\"devices\":[}").is_err());
    }

    #[test]
    fn optional_fields_default_to_empty() {
        let snapshot = parse(r#"{"devices":[{"id":"node-9"}]}"#).expect("payload parses");
        let device = &snapshot.devices[0];

        assert_eq!(device.hostname, "");
        assert_eq!(device.dns_name, "");
        assert!(device.addresses.is_empty());
        assert!(device.tags.is_empty());
        assert_eq!(device.last_seen, None);
        assert!(!device.online);
        assert!(!device.authorized);
    }

    #[test]
    fn unparseable_last_seen_is_offline_without_failing() {
        let snapshot =
            parse(r#"{"devices":[{"id":"node-9","lastSeen":"yesterday"}]}"#).expect("parses");

        assert_eq!(snapshot.devices[0].last_seen, None);
        assert!(!snapshot.devices[0].online);
    }

    #[test]
    fn online_uses_the_last_seen_threshold() {
        let now = collected_at();
        let fresh = now - Duration::seconds(ONLINE_THRESHOLD_SECONDS - 1);
        let boundary = now - Duration::seconds(ONLINE_THRESHOLD_SECONDS);
        let stale = now - Duration::seconds(ONLINE_THRESHOLD_SECONDS + 1);

        assert!(is_online(Some(fresh), now));
        assert!(!is_online(Some(boundary), now));
        assert!(!is_online(Some(stale), now));
        assert!(!is_online(None, now));
    }
}
