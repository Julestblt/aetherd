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
- [ ] Optional TOML configuration layered with `AETHERD_` environment variables
- [ ] Structured `tracing` setup with `RUST_LOG` filtering
- [ ] Graceful shutdown on SIGINT/SIGTERM

## 2. System telemetry collectors

- [ ] Host information (hostname, OS, kernel, architecture, boot time)
- [ ] CPU times and since-boot utilization from `/proc/stat`
- [ ] Memory and swap from `/proc/meminfo`
- [ ] Load average and process counts from `/proc/loadavg`
- [ ] Uptime and idle time from `/proc/uptime`
- [ ] Filesystems and usage from `/proc/mounts` + `statvfs`
- [ ] Network interface RX/TX from `/proc/net/dev`
- [ ] Interval-based CPU sampling (next improvement on top of since-boot)
- [ ] Disk I/O from `/proc/diskstats`
- [ ] Temperatures from `/sys/class/hwmon` and `/sys/class/thermal`
- [ ] Per-core and per-NUMA CPU breakdown
- [ ] Process information and counts
- [ ] Docker/container telemetry
- [ ] Configurable collector enable/disable

## 3. API

- [ ] Versioned `/v1` route structure
- [x] `GET /health` liveness endpoint
- [ ] `GET /v1/system` aggregated overview with partial-data handling
- [ ] `GET /v1/system/host`
- [ ] `GET /v1/system/cpu`
- [ ] `GET /v1/system/memory`
- [ ] `GET /v1/system/load`
- [ ] `GET /v1/system/uptime`
- [ ] `GET /v1/system/disks` with explicit pseudo-filesystem filtering
- [ ] `GET /v1/system/network`
- [x] Machine-readable, consistent error schema
- [ ] Consistent units and RFC3339 UTC timestamps documented in schemas
- [ ] Optional authentication for exposed deployments
- [ ] Request rate limiting

## 4. OpenAPI documentation

- [x] Compile-time generated OpenAPI from Rust types
- [x] `/openapi.json` always available, independent of the UI
- [ ] Swagger UI behind the default-on `swagger-ui` feature
- [ ] Documented Postman import from the OpenAPI spec
- [ ] API-level golden tests pinning the spec

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

- [ ] Multi-stage `Dockerfile` producing a small runtime image
- [ ] `compose.yaml` with read-only `/proc`, `/sys`, and `/` mounts
- [ ] Non-root runtime user; no `--privileged`
- [ ] Container health check hook (`aetherd healthcheck`)
- [ ] Multi-architecture image builds in CI

## 7. Observability and security

- [ ] Structured request tracing with latency and status
- [ ] No secret logging; redacting `SecretString` type
- [ ] Prometheus metrics endpoint
- [ ] Optional API authentication and TLS termination guidance
- [ ] Dependency and supply-chain gate (`cargo deny`)

## 8. Testing

- [ ] Unit tests for every parser (valid, malformed, missing, partial)
- [x] Integration tests over the Axum router without a live server
- [ ] Deterministic fixtures via configurable filesystem roots
- [x] HTTP status code and error schema regression coverage
- [ ] Property tests for parsers with wide input spaces
- [ ] API response snapshot tests for contract stability

## 9. Documentation

- [ ] `README.md` describing status, usage, and configuration
- [ ] `docs/architecture.md`
- [ ] `docs/api.md` including Postman import
- [ ] `docs/configuration.md`
- [ ] Deployment guide for common reverse proxies
