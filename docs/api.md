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
| GET | `/v1/system/stream` | Server-Sent Events stream of the latest snapshot. |

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
