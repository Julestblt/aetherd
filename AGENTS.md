# AGENTS.md

Persistent engineering instructions for agents working in this repository.
Read `docs/agents/rust.md` before running any verification step; it records the
toolchain posture and takes precedence over skill defaults.

## Project

`aetherd` is a small, headless Rust daemon that exposes Linux system telemetry
and, later, normalized AI provider usage through a versioned HTTP API. It must
run both directly on a host and inside a container with read-only host mounts.

## Commands

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
cargo build --all-features
```

`--no-default-features` must also build: Swagger UI is an optional feature.
Verify against the MSRV (1.88) when touching dependency or std usage.

## Code rules

- All code, identifiers, docs, and commit messages are in English.
- Do not add explanatory comments to source code. Use expressive names, small
  functions, clear types, and module boundaries instead. Rustdoc comments on
  public items are required and are part of the API contract.
- No `unwrap()` or `expect()` in production paths; errors are explicit and
  structured. Panics are only for violated internal invariants.
- Do not introduce `unsafe`; `#![forbid(unsafe_code)]` is set at both roots.
- No generic `utils`/`helpers` modules. Place code in the domain it serves.
- Never log secrets, tokens, authorization headers, or sensitive environment
  variables. Use structured `tracing` fields, never interpolated sentences.
- Keep files cohesive and small; split a module when it starts mixing
  responsibilities.

## Architecture

```
src/config/     layered configuration (defaults -> optional TOML -> env)
src/system/     Linux telemetry domain: typed metrics + /proc,/sys parsers
src/providers/  (planned) modular AI usage providers behind a registry
src/api/        HTTP contract: versioned routes, schemas, error responses
src/app.rs      composition root: AppState + router assembly
src/main.rs     binary: config, tracing, runtime, graceful shutdown
```

Extension seams:
- A new system metric is a new `system` collector module plus an `api` handler;
  it must not change unrelated collectors.
- Filesystem access goes through `system::SystemPaths` so tests can point at
  fixtures instead of the developer's machine.
- Disk statistics are injected through the `MountStats` trait so tests are
  deterministic.
- AI providers will implement an internal provider trait and register in a
  registry; an unavailable provider must never make the daemon unavailable.

## Workflow

- `TODO.md` is the canonical roadmap. Before starting work, read it; when a
  feature is implemented and tested, check off the relevant items and add any
  newly discovered follow-up work. Do not scatter TODO comments in source.
- Every feature is complete only when it is implemented, tested (including
  error and partial-data behavior), documented when user-facing, reflected in
  `TODO.md`, formatted, lint-clean, and committed independently.
- Commit messages use English Conventional Commits. Do not squash or rewrite
  history unless explicitly asked.
- Tests must be deterministic: never depend on the developer's `/proc`, `/sys`,
  network, or wall clock. Use fixtures and injected state.

## Skills

The vendored skills in `.agents/skills/` cover Rust craft (errors, testing,
async, observability, API design). Project-local skills in the same directory
cover recurring aetherd workflows (adding a collector, an endpoint, a provider).
Consult them rather than duplicating their guidance here.
