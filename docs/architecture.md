# Architecture

`aetherd` is deliberately small. The architecture exists to keep two promises:
new metrics and new providers can be added without touching unrelated code, and
partial or failing data never takes the daemon down.

## Layers

```
                 +------------------------------+
   HTTP request  |  api  (routes, schemas, errors)
                 +------------------------------+
                                |
                 +------------------------------+
                 |  app.rs  (AppState, router)  |
                 +------------------------------+
                    |                       |
        +---------------------+   +-----------------------+
        |  system collectors  |   |  providers (planned)  |
        +---------------------+   +-----------------------+
                    |                       |
              SystemPaths                remote APIs
        (/proc, /sys, host root)
```

- `config` produces a validated `Config` and never performs I/O after load.
- `app.rs` builds `AppState` and the router. It is the only place that knows
  every collector.
- `api` owns the HTTP contract: path layout, request/response schemas, error
  mapping, and OpenAPI annotations. Handlers translate domain errors into
  status codes; they do no parsing themselves.
- `system` owns Linux telemetry: typed metrics plus parsers for `/proc` and
  `/sys` content. It knows nothing about HTTP.
- `providers` (planned) owns AI usage normalization behind a trait and registry.

## The collector seam

Each system metric is a module in `src/system/` exposing:

- a pure parser over text (`parse_meminfo(&str) -> Result<Metric, ParseError>`)
  so edge cases are tested without touching a filesystem, and
- a collector type implementing

  ```rust
  pub trait SystemCollector {
      type Metric;
      fn name(&self) -> &'static str;
      fn collect(&self, paths: &SystemPaths) -> Result<Self::Metric, CollectorError>;
  }
  ```

Adding a metric means adding one module, one handler, one route, and the schema
registration. It does not change other collectors.

## Filesystem roots

All file access goes through `SystemPaths { proc, sys, host_root }`. A path such
as `/proc/stat` is resolved as `paths.proc.join("stat")`, and a host-absolute
path such as `/etc/os-release` is resolved under `host_root`. This is what makes
container deployments (`/host/proc`, `/host/sys`, `/host/root`) and deterministic
tests possible with the same code.

Disk statistics are the exception that proves the rule: `statvfs` is a syscall,
not a file read, so it is injected through the `MountStats` trait. Production
wires the real implementation; tests wire a fake.

## Partial data

A metrics daemon reads many independent sources, and any of them can fail on a
given host. The API therefore distinguishes "the request failed" from "one
section is unavailable":

- Per-metric endpoints return `200` with the metric, or `503` with a structured
  error when that single metric cannot be collected.
- The aggregate `GET /v1/system` always returns `200` and marks each section as
  available or unavailable with a reason. One missing sensor never fails the
  overview.

## Units and time

- Byte counts are `u64` bytes.
- Durations and load are `f64` seconds.
- Utilization is a `f64` percentage in `0..=100`.
- Counters are `u64` values accumulated since boot.
- Timestamps are RFC3339 UTC strings.

These conventions are part of the schema descriptions so the OpenAPI document is
the single source of truth.

## AI provider seam (planned)

Providers will implement a small internal trait and register in a registry held
by `AppState`. Each provider reports its own availability and errors; the
aggregate usage endpoint collects successful results and marks the rest
unavailable. Credentials come from the environment only, and are represented by
a redacting `SecretString` so they cannot be logged by accident.

No provider code is written until the normalized usage model is defined; the
roadmap in `TODO.md` tracks that work.
