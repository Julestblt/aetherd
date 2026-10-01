# HTTP API

`aetherd` serves a JSON API. Telemetry lives under the versioned prefix `/v1`;
health is unversioned.

- OpenAPI document: `GET /openapi.json`
- Swagger UI: `GET /swagger-ui` (when the default `swagger-ui` feature is on)

## Conventions

- **Timestamps** are RFC3339 UTC strings, for example `2026-09-25T15:00:00Z`.
- **Byte counts** are `u64` bytes.
- **Durations, load, and idle time** are `f64` seconds.
- **Utilization** is a `f64` percentage in `0..=100`.
- **Rates** are `f64` bytes per second.
- **CPU ticks** are `u64` clock ticks in `USER_HZ` (conventionally 100/second).
- **Counters** are `u64` values accumulated since boot.
- **Interval-derived** values (`interval_usage_percent`, `rx_bytes_per_second`,
  `tx_bytes_per_second`) are `null` on the first sample, after a counter reset,
  and for a source seen for the first time. They are never replaced by a
  synthetic zero.

## Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/health` | Liveness, version, and daemon uptime. Always `200`. |
| GET | `/v1/system` | Aggregated overview; each section reports availability. Always `200`. |
| GET | `/v1/system/host` | Hostname, OS, kernel, architecture, boot time. Never fails on missing metadata. |
| GET | `/v1/system/cpu` | Aggregate and per-core CPU times and since-boot utilization. |
| GET | `/v1/system/memory` | RAM and swap usage. |
| GET | `/v1/system/load` | Load average and process counts. |
| GET | `/v1/system/uptime` | System uptime and idle time. |
| GET | `/v1/system/disks` | Mounted filesystems and capacity, excluding pseudo-filesystems. |
| GET | `/v1/system/network` | Per-interface RX/TX counters and interval rates. |
| GET | `/v1/system/tailscale` | Tailnet machines from the last Tailscale refresh. |
| GET | `/v1/system/stream` | Server-Sent Events stream of the latest snapshot. |
| GET | `/v1/providers` | Codex and OpenCode Go configuration and refresh status. |
| GET | `/v1/usage` | Last successful normalized quota data from enabled providers. |

## AI usage

`GET /v1/providers` always reports the Codex and OpenCode Go slots. Each has
`id`, `display_name`, `enabled`, `status` (`disabled`, `unavailable`, or
`available`), `last_updated_at`, and an optional secret-free `error`.
`last_updated_at` is the last successful refresh, including when a later
refresh has failed.

`GET /v1/usage` returns `collected_at` and a `providers` array. Only providers
with a successful cached value appear in that array. A later failure keeps the
last successful value, while `/v1/providers` marks that provider unavailable.
Clients should use the status and timestamp together to identify stale values.
`collected_at` is the newest successful provider refresh time, or `null` when
none has succeeded. Both endpoints return `200` when one provider fails.

Each provider result has `provider_id`, `display_name`, optional
`account_label`, and `windows`. A window has `kind`, `label`, and optional
`duration_seconds`, `used_percent`, `remaining_percent`, `resets_at`, `tokens`,
`requests`, and `cost_usd`. Optional `totals` and `models` allow future model
accounting, but neither upstream quota source currently supplies those counts.
Percentages are in `0..=100` and timestamps are RFC3339 UTC. Unsupported
values are omitted, never estimated.

Codex currently exposes a primary and secondary quota window with used
percentage, duration, and reset time. The 5-hour and 7-day durations are
identified as `session` and `weekly`; other durations are `custom`. OpenCode Go
exposes rolling 5-hour, weekly, and monthly percentages and reset times. Its
weekly and monthly duration is omitted because the source gives only the reset
time. The generic OpenCode runtime can use other model providers; its local
session counters are not an OpenCode Go account quota.

Provider collection refreshes independently of system sampling and Tailscale.
There is no usage SSE endpoint; consumers can poll these REST routes around
once per minute. Provider values never enter `/v1/system` or its stream.

Per-metric endpoints return `200` with the latest sampled section, or `503`
with a structured error when the snapshot is not ready yet or that metric is
unavailable.

## Sampling and freshness

A single background task samples the complete system on a fixed interval
(one second by default, see [configuration](configuration.md)) and stores the
latest `SystemSnapshot` in memory. REST reads that snapshot; it does not reread
`/proc` or call `statvfs` per request. `/v1/system` returns the whole snapshot;
metric endpoints return the matching section.

Because sampling is interval-based, some values need a previous sample:

- `cpu.interval_usage_percent` (and per core) is the utilization over the
  interval between the two most recent samples. `cpu.usage_percent` remains the
  cumulative average since boot.
- `network.interfaces[].rx_bytes_per_second` and `tx_bytes_per_second` are
  derived from counter deltas over the same interval. The cumulative `rx_bytes`
  and `tx_bytes` counters are still reported.

If the daemon has not produced its first snapshot yet, every system endpoint
returns `503` with error code `not_ready` instead of inventing data.

## Tailscale telemetry

Tailscale machines are optional and read from the official Tailscale HTTP API
(`GET /api/v2/tailnet/{tailnet}/devices`); no `tailscale` CLI or host socket is
required. The integration stays off unless `tailscale.enabled` is true and both
a tailnet and an API key are configured. A missing or disabled configuration is
not an error: the daemon starts, and the `tailscale` section reports why it is
unavailable.

Tailscale refreshes on its own schedule (`tailscale.refresh_interval_seconds`,
`60` seconds by default) and is never fetched once per system sample. The
sampler reads the latest cached Tailscale section, so `GET /v1/system`,
`GET /v1/system/tailscale`, and `GET /v1/system/stream` keep serving the most
recent refresh even though system metrics update once per second.

A failed refresh (timeout, `401`/`403`, invalid JSON, unreachable host) replaces
the section with `unavailable` and its reason until the next successful refresh.
The daemon stays healthy and every other section keeps updating.

Device fields are a small, stable projection of the upstream object:

```json
{
  "id": "node-1",
  "hostname": "homelab",
  "dns_name": "homelab.example.ts.net",
  "os": "linux",
  "addresses": ["100.64.0.1"],
  "online": true,
  "last_seen": "2026-09-25T22:13:20Z",
  "authorized": true,
  "tags": ["tag:homelab"]
}
```

The Tailscale API exposes no online flag, so `online` is inferred from
`last_seen`: a device is online when its last handshake is less than 120 seconds
before the refresh timestamp. The threshold is a single constant
(`ONLINE_THRESHOLD_SECONDS` in `src/tailscale/model.rs`), and it is an
approximation, not an authoritative state. `last_seen` is always reported so a
client can distinguish online, recently seen, and offline; an absent or
unparseable timestamp becomes `null` and counts as offline.

## Live stream

`GET /v1/system/stream` is a Server-Sent Events stream with
`Content-Type: text/event-stream`. Each event is named `system` and its `data`
is a `SystemSnapshot` serialized as JSON:

```text
event: system
data: {"collected_at":"2026-09-25T22:13:20Z","host":{...},...}
```

- A client receives the current snapshot immediately on connect, then one event
  per new sample.
- Clients that read slowly miss intermediate samples rather than queueing them:
  the stream always delivers the latest value, so per-client memory stays
  bounded.
- A disconnected or slow client never affects the sampler or other clients.
- A keep-alive comment is sent periodically, and shutdown terminates open
  streams cleanly.

## Partial data

`GET /v1/system` never fails because one source is missing. Each section is one
of:

```json
{ "status": "available", "value": { } }
```

```json
{ "status": "unavailable", "reason": "cannot read /host/proc/stat" }
```

This means a missing temperature sensor or an unreadable disk never hides the
sections that did succeed.

## Filesystem filtering

`GET /v1/system/disks` collects every mount internally but omits pseudo- and
kernel-internal filesystems from the response: `proc`, `sysfs`, `cgroup`,
`cgroup2`, `devtmpfs`, `devpts`, `tmpfs`, `overlay`, `debugfs`, `tracefs`,
`securityfs`, `pstore`, `bpf`, `configfs`, `mqueue`, `hugetlbfs`, `fusectl`,
`autofs`, `binfmt_misc`, `efivarfs`, `nsfs`, and `ramfs`. The
filter list is a single constant in `src/system/disks.rs`; changing the policy
is a one-line change with tests in the same module.

## Errors

Every failing request returns the same envelope:

```json
{
  "error": {
    "code": "unavailable",
    "message": "cannot read /host/proc/stat"
  }
}
```

Codes are stable and machine-readable: `not_found`, `method_not_allowed`,
`not_ready`, `unavailable`. Messages never expose secrets or internal
implementation details.

## OpenAPI and Swagger UI

The OpenAPI 3.1 document is generated from the Rust types, so it cannot drift
from the handlers. It is always available at `/openapi.json`, independently of
Swagger UI. When the `swagger-ui` feature is enabled (it is on by default),
Swagger UI is served at `/swagger-ui` and reads the same document.

## Postman

No second contract is maintained. Import the generated OpenAPI document into
Postman:

1. Start the daemon, or fetch the spec from a deployed instance.
2. In Postman choose **Import** → **File** and select a saved `openapi.json`, or
   choose **Import** → **Link** and enter `http://localhost:8080/openapi.json`.
3. Postman generates a collection with every endpoint and schema.

Because the collection is generated from `/openapi.json`, regenerating it after
an API change keeps Postman in sync without hand-editing.
