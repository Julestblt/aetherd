---
name: aetherd-add-ai-provider
description: Add an AI usage provider to aetherd behind the provider seam, with normalized usage types, per-provider isolation, environment-only credentials, deterministic fake-backed tests, and the /v1/providers and /v1/usage surface. Use when implementing or reviewing AI provider support.
---

# Add an AI usage provider

Read `AGENTS.md`, `docs/agents/rust.md`, and the provider section of
`docs/architecture.md` first. No provider code exists yet; this skill defines the
shape to build so the daemon stays available when a provider is not.

## Non-negotiable constraints

- A provider being disabled, unauthenticated, misconfigured, rate-limited, or
  down must never affect daemon availability or the system-telemetry API.
- The core domain must not depend on any single provider. Providers map their
  own response into one normalized usage model.
- Credentials come from the environment only, are represented by a redacting
  `SecretString` (custom `Debug`/`Display` that print `<redacted>`), and are
  never logged or committed.

## 1. Domain model first

Define the normalized usage types before any client code: provider identity,
account or plan, quota windows, counters with explicit units, and a
retrieved-at timestamp. Keep provider-specific shapes out of the public model.

## 2. Provider seam

- Put providers in `src/providers/`.
- Define a small internal trait: a stable name, an availability/health check,
  and a fetch that returns the normalized model or a structured error.
- Hold providers in a registry on `AppState`. Enabled providers are configured,
  not hardcoded; a disabled or absent provider simply does not appear.
- One provider's error stays that provider's error. Aggregate endpoints collect
  successes and mark the rest unavailable, using `api::section::Section`.

## 3. HTTP surface

- `GET /v1/providers` lists configured providers and their availability.
- `GET /v1/usage` returns normalized usage across enabled providers.
- Follow `.agents/skills/aetherd-add-api-endpoint` for routing, schemas,
  OpenAPI, errors, and snapshots.

## 4. Tests

- Unit-test the response-to-model mapping against captured fixtures.
- Use a fake provider implementing the trait; never call a real network in tests.
- Cover: provider unavailable, provider returning malformed data, and one
  provider failing while another succeeds.
- Assert that a failing provider still yields `200` at the aggregate level.

## 5. Documentation and roadmap

- Update `docs/api.md`, `docs/architecture.md` if the seam changed, and
  `.env.example` with the credential variable name (never a value).
- Check the provider items off in `TODO.md` and add follow-ups you found.

## 6. Verify

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
```
