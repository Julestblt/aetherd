---
name: aetherd-add-api-endpoint
description: Add or change an aetherd HTTP endpoint, keeping routes, typed schemas, the generated OpenAPI document, error behavior, tests and docs in sync. Use when adding a route, changing a response shape, or adding an error case.
---

# Add or change an API endpoint

Read `AGENTS.md` and `docs/agents/rust.md` first. The route, the schema, and the
documentation are one change; they land together or the contract drifts.

## 1. Define the response type

Put the response type in the relevant `src/api/` module (or the domain module it
serializes). It must derive `Serialize` and `ToSchema`, and every field carries a
doc comment stating its unit or format (bytes, seconds, percent, RFC3339).

## 2. Handler and route

- Add a `pub(crate) async fn` handler with `#[utoipa::path(get, path = "...")]`.
  Paths under `/v1/system` are declared relative to the nest, for example
  `path = "/cpu"`.
- Register it in `build_parts` in `src/app.rs` with `routes!(...)`.
- Register the path and any new schemas in `src/api/openapi.rs`.

## 3. Error behavior

- Failures go through `ApiError`; never construct an ad-hoc error response.
- A metric that cannot be collected is `ApiError::Unavailable(reason)`, which is
  `503` with code `unavailable`. Never leak secrets or internal paths beyond the
  metric source already exposed.
- Endpoints that aggregate must return `200` and mark sections unavailable
  (`api::section::Section`) rather than failing the whole request.

## 4. Tests

Add integration coverage in `tests/api_*.rs` asserting the status code and the
response shape, including the failure path. For contract changes, refresh the
snapshots:

```bash
INSTA_UPDATE=always cargo test --test api_openapi
INSTA_UPDATE=always cargo test --test api_regression
```

Read the generated diff before committing it; a snapshot change is a contract
change.

## 5. Documentation and roadmap

- Update `docs/api.md` (endpoint table and any convention).
- Check the item off in `TODO.md`.

## 6. Verify

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
cargo clippy --all-targets --no-default-features
```
