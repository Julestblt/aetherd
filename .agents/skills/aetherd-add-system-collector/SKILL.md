---
name: aetherd-add-system-collector
description: Add a new Linux system-telemetry collector to aetherd, wired through the SystemCollector seam and SystemPaths, with fixtures, parser tests, an API endpoint and OpenAPI schema. Use when adding a metric from /proc or /sys.
---

# Add a system collector

Read `AGENTS.md` and `docs/agents/rust.md` first. Every step below is required
for the feature to be complete; do not stop after the parser.

## 1. Collector module

Create `src/system/<metric>.rs` containing:

- A `pub(crate)` metric struct deriving `Clone, Debug, PartialEq, Serialize,
  ToSchema`. Put units in the field docs; they become OpenAPI descriptions.
- A pure parser `parse_<source>(&str) -> Result<Metric, ParseError>` that never
  touches the filesystem, so edge cases are testable as strings.
- A `#[derive(Debug)] pub(crate) struct <Metric>Collector;` implementing
  `SystemCollector`:

  ```rust
  fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError> {
      let path = paths.proc_file("stat");
      let content = super::read_file(path.clone())?;
      parse_cpu_stat(&content).map_err(|source| CollectorError::Parse { path, source })
  }
  ```

Resolve paths only through `SystemPaths` (`proc_file`, `sys_path`, `host_path`).
Never hardcode `/proc` or `/sys`.

Register the module in `src/system/mod.rs`.

## 2. Parser tests

In `#[cfg(test)] mod tests`, cover: a valid sample, malformed values, missing
required fields, empty input, and any partial/optional fields. Keep expected
values hand-written, not computed with the code under test.

## 3. Fixture

Add a realistic file under `tests/fixtures/proc/...` (or `.../sys/...`). The
integration helper `common::fixture_paths()` already points `SystemPaths` at
`tests/fixtures`.

## 4. HTTP surface

- Add a handler in `src/api/system.rs` using `probe(&<Metric>Collector, ...)`
  for per-metric endpoints.
- If the metric needs injected state (a syscall, a remote source), add it to
  `AppState` and access it like `state.mount_stats`.
- Register the route in `src/app.rs` (`build_parts`) and the path plus schema in
  `src/api/openapi.rs`.
- Declare the `#[utoipa::path(path = "/<metric>")]` relative to the
  `/v1/system` nest.

## 5. Integration tests

Add tests in `tests/api_system.rs`: a success case from fixtures asserting
status and schema, and an unavailable case with `/nonexistent` roots asserting
`503` and `error.code == "unavailable"`.

## 6. Documentation and roadmap

- Check the item off in `TODO.md`.
- Add the collector to the README collector table.
- Add the endpoint to `docs/api.md`.
- Refresh the OpenAPI snapshot: `INSTA_UPDATE=always cargo test --test api_openapi`.

## 7. Verify

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
```
