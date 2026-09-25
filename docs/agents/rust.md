# Rust posture

This file is authoritative for this repo: every agent working here reads it
before running a verification step, and it takes precedence over any default a
skill would otherwise assume. Keep it short enough to read in full.

## Detected facts

| Fact | Value | Source |
|---|---|---|
| Edition | 2024 | `[package].edition` |
| MSRV | 1.88 | `rust-version` (driven by `utoipa` 6) |
| Async runtime | tokio | `[dependencies]` |
| `no_std` | no — std is linked | crate roots |
| Unsafe policy | forbidden | `#![forbid(unsafe_code)]` in `src/lib.rs` and `src/main.rs` |

## Commands

```bash
cargo fmt --check                                                  # formatting
cargo clippy --all-targets --all-features                          # at the level in Cargo.toml
cargo test --all-features                                          # unit + integration tests
cargo build --all-features                                         # also the MSRV command on 1.88
```

`--all-features` is meaningful: it enables the default-on `swagger-ui` feature
and exercises the Swagger UI integration. The crate must also build with
`--no-default-features`, because Swagger UI is optional.

Tooling: `cargo audit` — run in CI. `cargo hack`, `cargo udeps`, `cargo miri`
— not run; Miri is irrelevant while unsafe is forbidden.

## Posture

Unsafe is forbidden at the crate roots, so no `unsafe` block is expected. The
public surface is the `aetherd` library, which exists mainly so integration
tests can build the router without a live server; it is not a published API
with semver guarantees yet.

Lints are configured in `[lints.rust]` and `[lints.clippy]` in `Cargo.toml`;
thresholds live in `clippy.toml`. Do not add `-D warnings` to a command that
already respects the configured level.
