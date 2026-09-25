# aetherd

`aetherd` is a small, headless Rust daemon that exposes Linux system telemetry
and, later, normalized AI provider usage through a clean, versioned HTTP API.
It runs directly on a host or inside a container with read-only host mounts,
without requiring `--privileged`.

## Status

Early development with a working system-telemetry slice. The daemon starts,
loads layered configuration, serves a versioned API with generated OpenAPI
documentation, and collects real metrics from `/proc`, `/sys`, and `statvfs`.

Implemented today:

- Configuration from defaults, an optional TOML file, and `AETHERD_` environment
  variables
- Structured `tracing`, graceful shutdown, and a Docker image
- `GET /health`
- `GET /v1/system` plus `host`, `cpu`, `memory`, `load`, `uptime`, `disks`, and
  `network` endpoints
- OpenAPI 3.1 generated from Rust types, with Swagger UI
- Deterministic tests over fixtures, with no dependency on the developer's host

Not implemented yet: AI usage providers, interval-based CPU sampling, disk I/O
counters, temperatures, and process/container telemetry. `TODO.md` is the
canonical roadmap and is the source of truth for what exists.

## Supported collectors

| Collector | Source | Endpoint |
|---|---|---|
| Host | `/proc/sys/kernel/*`, `{host_root}/etc/os-release` | `/v1/system/host` |
| CPU | `/proc/stat` | `/v1/system/cpu` |
| Memory | `/proc/meminfo` | `/v1/system/memory` |
| Load | `/proc/loadavg` | `/v1/system/load` |
| Uptime | `/proc/uptime` | `/v1/system/uptime` |
| Disks | `/proc/mounts` + `statvfs` | `/v1/system/disks` |
| Network | `/proc/net/dev` | `/v1/system/network` |

## Architecture

```
src/config/     layered configuration (defaults -> optional TOML -> env)
src/system/     Linux telemetry domain: typed metrics + /proc,/sys parsers
src/providers/  (planned) modular AI usage providers behind a registry
src/api/        HTTP contract: versioned routes, schemas, error responses
src/app.rs      composition root: AppState + router assembly
src/main.rs     binary: config, tracing, runtime, graceful shutdown
```

Key ideas:

- **Injectable filesystem roots.** Collectors read through `SystemPaths`, so the
  daemon can read `/proc`, `/sys`, and a host root from configurable locations,
  and tests can point the same type at fixtures.
- **Partial data is normal.** A missing sensor or mount never fails an aggregate
  response; per-section availability is explicit.
- **Providers stay isolated.** An unavailable or failing AI provider will never
  make the daemon unavailable.

See [`docs/architecture.md`](docs/architecture.md) for the long form.

## Requirements

- Rust 1.88 or newer (edition 2024)
- Linux for system telemetry

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
cargo build --all-features
```

`--no-default-features` must also build: Swagger UI is optional.

## Running

```bash
cargo run
```

The server listens on `127.0.0.1:8080` by default. Use `--config <PATH>` to load
a TOML file, or configure entirely through environment variables:

```bash
AETHERD_HTTP__BIND=0.0.0.0:8080 cargo run
```

## Configuration

Configuration is defaults, then an optional TOML file, then `AETHERD_`-prefixed
environment variables. See [`docs/configuration.md`](docs/configuration.md) and
[`.env.example`](.env.example).

## API documentation

The OpenAPI document is generated from the Rust types and served at
`/openapi.json`; Swagger UI is at `/swagger-ui`. Endpoint details, units, error
codes, partial-data representation, and Postman import steps are in
[`docs/api.md`](docs/api.md).

## Docker

```bash
docker build -t aetherd:local .
docker run --rm -p 8080:8080 \
  -v /proc:/host/proc:ro \
  -v /sys:/host/sys:ro \
  -v /:/host/root:ro \
  -e AETHERD_HTTP__BIND=0.0.0.0:8080 \
  aetherd:local
```

`compose.yaml` provides the same setup. The image runs as a non-root user with
read-only host mounts and no added capabilities.

## Roadmap

[`TODO.md`](TODO.md) is the canonical roadmap.

## Releases

Releases are tag-driven: pushing a `vX.Y.Z` tag verifies the tree, publishes a
GitHub Release with a binary archive and checksums, and pushes a container image
to GHCR. See [`CHANGELOG.md`](CHANGELOG.md) for released changes and
[`RELEASING.md`](RELEASING.md) for the process.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option.
