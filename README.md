# aetherd

`aetherd` is a small, headless Rust daemon that exposes Linux system telemetry
and, later, normalized AI provider usage through a clean, versioned HTTP API.
It is designed to run directly on a host or inside a container with read-only
host mounts, without requiring `--privileged`.

## Status

Early development. The repository currently contains the project foundation:
the Rust crate, lint and format configuration, the documented toolchain posture
(`docs/agents/rust.md`), and the architecture below. Collectors and API
endpoints are being implemented; see `TODO.md` for the canonical roadmap.

Documentation describes what exists today. Nothing in this README should be
read as an implemented feature until it is checked off in `TODO.md`.

## Architecture

`aetherd` is organized around domain boundaries rather than generic helper
modules:

```
src/config/     layered configuration (defaults -> optional TOML -> env)
src/system/     Linux telemetry domain: typed metrics + /proc,/sys parsers
src/providers/  (planned) modular AI usage providers behind a registry
src/api/        HTTP contract: versioned routes, schemas, error responses
src/app.rs      composition root: AppState + router assembly
src/main.rs     binary: config, tracing, runtime, graceful shutdown
```

Key ideas:

- **Injectable filesystem roots.** Every collector reads through
  `system::SystemPaths`, so the daemon can read `/proc`, `/sys`, and a host
  root from configurable locations. Tests point the same type at fixtures.
- **Partial data is normal.** A missing sensor or mount must not fail an
  aggregate response; per-section availability is represented explicitly.
- **Providers are isolated.** An unavailable, unauthenticated, or failing AI
  provider must never make the daemon unavailable.

See [`docs/architecture.md`](docs/architecture.md) for the long form.

## Requirements

- Rust 1.88 or newer (edition 2024)
- Linux for system telemetry (the daemon itself builds anywhere)

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
cargo build --all-features
```

`--no-default-features` must also build: Swagger UI is an optional feature.

## Running

```bash
cargo run
```

Configuration is optional. Built-in defaults plus `AETHERD_`-prefixed
environment variables are sufficient; a TOML file can be layered on top. See
`.env.example`.

## API documentation

The OpenAPI specification is generated from the Rust types and is always served
at `/openapi.json`. Swagger UI is available at `/swagger-ui` when the
default-on `swagger-ui` feature is enabled.

## Docker

Build and run the container:

```bash
docker build -t aetherd:local .
docker run --rm -p 8080:8080 \
  -v /proc:/host/proc:ro \
  -v /sys:/host/sys:ro \
  -v /:/host/root:ro \
  -e AETHERD_HTTP__BIND=0.0.0.0:8080 \
  aetherd:local
```

`compose.yaml` provides the same configuration. The container runs as a
non-root user with read-only host mounts, no added capabilities, and does not
require `--privileged`.

## Roadmap

[`TODO.md`](TODO.md) is the canonical roadmap.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option.
