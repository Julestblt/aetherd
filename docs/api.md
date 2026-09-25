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
- **CPU ticks** are `u64` clock ticks in `USER_HZ` (conventionally 100/second).
- **Counters** are `u64` values accumulated since boot.

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
| GET | `/v1/system/network` | Per-interface RX/TX counters. |

Per-metric endpoints return `200` with the metric, or `503` with a structured
error when that metric cannot be collected on the current host.

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
`unavailable`. Messages never expose secrets or internal implementation details.

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
