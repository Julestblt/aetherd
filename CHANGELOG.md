# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Background system sampler with a configurable interval
  (`AETHERD_SAMPLING__INTERVAL_MS`, default `1000`), publishing into an
  in-memory latest-snapshot cache.
- `GET /v1/system/stream`, a Server-Sent Events stream (`event: system`) that
  sends the current snapshot on connect and each new sample afterwards.
- Interval-based CPU utilization (`interval_usage_percent` on the aggregate and
  every core) and network RX/TX rates (`rx_bytes_per_second`,
  `tx_bytes_per_second`) derived from consecutive samples.
- `not_ready` error code returned when no snapshot has been sampled yet.

### Changed

- REST endpoints now read the latest sampled snapshot instead of collecting on
  every request. Response shapes are unchanged except for the additive interval
  fields.
- The generated OpenAPI document no longer advertises non-functional
  unprefixed system paths; nested routes appear only under `/v1/system`.

## [0.1.0] - 2026-09-25

First release. A working Linux system-telemetry slice; AI provider support is
planned and not part of this release.

### Added

- Layered configuration: built-in defaults, an optional TOML file
  (`--config`, or `aetherd.toml`), and `AETHERD_`-prefixed environment variables
  with `__` for nested keys.
- Structured `tracing` with `RUST_LOG` filtering and graceful SIGINT/SIGTERM
  shutdown.
- `GET /health` liveness endpoint.
- System telemetry endpoints: `GET /v1/system` (aggregated overview with
  per-section availability), `/v1/system/host`, `/cpu`, `/memory`, `/load`,
  `/uptime`, `/disks`, and `/network`.
- Collectors backed by `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`,
  `/proc/uptime`, `/proc/mounts` + `statvfs`, `/proc/net/dev`, and
  `/proc/sys/kernel/*` plus `/etc/os-release`.
- Filesystem metrics exclude pseudo-filesystems by default, through one
  documented filter constant.
- Machine-readable, consistent error envelope (`not_found`,
  `method_not_allowed`, `unavailable`) with stable HTTP status codes.
- OpenAPI 3.1 generated from Rust types at `/openapi.json`, always available,
  with Swagger UI behind the default-on `swagger-ui` feature.
- Injectable filesystem roots (`/proc`, `/sys`, host root) and an injectable
  `MountStats` seam for deterministic tests.
- Deterministic unit and integration tests over fixtures, including parser edge
  cases, partial data, error schemas, and contract snapshots.
- Multi-stage Docker image running as a non-root user with read-only host
  mounts, plus a `compose.yaml` example.
- CI workflow covering formatting, clippy, tests, the Rust 1.88 MSRV build, and
  dependency auditing.

[Unreleased]: https://github.com/Julestblt/aetherd/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Julestblt/aetherd/releases/tag/v0.1.0
