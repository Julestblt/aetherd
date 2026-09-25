# aetherd roadmap

This file is the canonical implementation roadmap. It is updated as work lands.
Completed items are kept for historical context. A feature is only checked off
when it is implemented and tested.

Status legend: `[x]` done and tested, `[ ]` not started, `[~]` partially done.

## 1. Project foundation

- [x] Initialize Rust crate with `lib` + `bin` layout
- [x] Pin edition 2024 and MSRV 1.88; record posture in `docs/agents/rust.md`
- [x] Configure rustfmt, clippy, and rustc lint levels
- [x] Establish architecture and agent instructions (`AGENTS.md`)
- [x] Optional TOML configuration layered with `AETHERD_` environment variables
- [x] Structured `tracing` setup with `RUST_LOG` filtering
- [x] Graceful shutdown on SIGINT/SIGTERM

## 2. System telemetry collectors

- [x] Host information (hostname, OS, kernel, architecture, boot time)
- [x] CPU times and since-boot utilization from `/proc/stat`
- [x] Memory and swap from `/proc/meminfo`
- [x] Load average and process counts from `/proc/loadavg`
- [x] Uptime and idle time from `/proc/uptime`
- [x] Filesystems and usage from `/proc/mounts` + `statvfs`
- [x] Network interface RX/TX from `/proc/net/dev`
- [ ] Interval-based CPU sampling (next improvement on top of since-boot)
- [ ] Disk I/O from `/proc/diskstats`
- [ ] Temperatures from `/sys/class/hwmon` and `/sys/class/thermal`
- [ ] Per-core and per-NUMA CPU breakdown
- [ ] Process information and counts
- [ ] Docker/container telemetry
- [ ] Configurable collector enable/disable

## 3. API

- [x] Versioned `/v1` route structure
- [x] `GET /health` liveness endpoint
- [x] `GET /v1/system` aggregated overview with partial-data handling
- [x] `GET /v1/system/host`
- [x] `GET /v1/system/cpu`
- [x] `GET /v1/system/memory`
- [x] `GET /v1/system/load`
- [x] `GET /v1/system/uptime`
- [x] `GET /v1/system/disks` with explicit pseudo-filesystem filtering
- [x] `GET /v1/system/network`
- [x] Machine-readable, consistent error schema
- [x] Consistent units and RFC3339 UTC timestamps documented in schemas
- [ ] Optional authentication for exposed deployments
- [ ] Request rate limiting
- [ ] Request timeout with a structured error body
- [ ] Reject unknown `AETHERD_` environment variables with a clear message (currently rejected by serde)

## 4. OpenAPI documentation

- [x] Compile-time generated OpenAPI from Rust types
- [x] `/openapi.json` always available, independent of the UI
- [x] Swagger UI behind the default-on `swagger-ui` feature
- [ ] Documented Postman import from the OpenAPI spec
- [x] API-level golden tests pinning the spec

## 5. AI usage providers

- [ ] Define normalized usage domain model
- [ ] Internal provider trait and registry with per-provider isolation
- [ ] `GET /v1/providers` listing configured providers and status
- [ ] `GET /v1/usage` normalized usage across enabled providers
- [ ] OpenAI / Codex provider
- [ ] Anthropic / Claude provider
- [ ] OpenRouter provider
- [ ] OpenCode / OpenCode Go provider
- [ ] Per-provider credentials from the environment only
- [ ] Provider-level timeouts, backoff, and caching

## 6. Docker

- [x] Multi-stage `Dockerfile` producing a small runtime image
- [x] `compose.yaml` with read-only `/proc`, `/sys`, and `/` mounts
- [x] Non-root runtime user; no `--privileged`
- [ ] Container health check hook (`aetherd healthcheck`)
- [ ] Multi-architecture image builds in CI

## 7. Observability and security

- [x] Structured request tracing with latency and status
- [ ] No secret logging; redacting `SecretString` type
- [ ] Prometheus metrics endpoint
- [ ] Optional API authentication and TLS termination guidance
- [ ] Dependency and supply-chain gate (`cargo deny`)

## 8. Testing

- [x] Unit tests for every parser (valid, malformed, missing, partial)
- [x] Integration tests over the Axum router without a live server
- [x] Deterministic fixtures via configurable filesystem roots
- [x] HTTP status code and error schema regression coverage
- [ ] Property tests for parsers with wide input spaces
- [x] API response snapshot tests for contract stability

## 10. Documentation

- [ ] `README.md` describing status, usage, and configuration
- [x] `docs/architecture.md`
- [ ] `docs/api.md` including Postman import
- [ ] `docs/configuration.md`
- [ ] Deployment guide for common reverse proxies

## 11. Continuous integration

- [x] GitHub Actions workflow running format, clippy, tests, and MSRV build
- [x] Dependency advisory audit (`cargo audit`)
- [ ] Scheduled feature-powerset check (`cargo hack`)
- [ ] Multi-architecture build matrix
