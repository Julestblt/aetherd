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
        |  system collectors  |   |  usage providers      |
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
- `providers` owns AI usage normalization behind a trait and independent refresh tasks.

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

## Sampling and distribution

Collecting every metric on every request does not scale and makes live updates
awkward, so collection is decoupled from serving:

```
/proc + /sys + statvfs
        |
  background sampler (one task, fixed interval)
        |
  SystemSnapshot (partial sections, interval metrics)
        |
  tokio::sync::watch<Option<Arc<SystemSnapshot>>>
        |
        +--> REST reads the latest snapshot
        +--> SSE subscribers receive each new snapshot
```

- `sampling::SnapshotBuilder` performs one synchronous collection pass, keeping
  the previous CPU and network readings so interval-derived values can be
  computed. It takes the sample time as an argument, which keeps it
  deterministic and testable.
- `sampling::spawn_sampler` runs a single loop: collect, publish, wait for the
  next tick or shutdown. There is no worker pool, and the task stops on
  application shutdown.
- The shared value is an `Option<Arc<SystemSnapshot>>`. `None` means "no sample
  yet" and is reported as a `not_ready` error rather than placeholder data.
  `Arc` lets readers share the snapshot without cloning it.
- `watch` keeps only the latest value, so a slow SSE client misses intermediate
  samples instead of accumulating an unbounded queue.

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

## AI provider seam

Codex and OpenCode Go implement an internal `UsageProvider` trait. Enabled
providers each run a refresh task on their configured interval, independent of
the system sampler and Tailscale. `AppState` holds a latest-value cache of
provider status and successful normalized usage. Failed refreshes mark only
that provider unavailable and retain its previous successful value, with the
last successful timestamp for freshness decisions.

`GET /v1/providers` reads status; `GET /v1/usage` reads only successful cached
values. The provider model contains optional quota windows and accounting
fields, so absent token, request, model, and cost data are not invented.
Codex credential file paths come from configuration or narrow standard
fallbacks; the selected file is read on each refresh. OpenCode instead uses a
Console service-account API key in redacting `SecretString`. Neither raw auth
data nor upstream response bodies are exposed.

Codex reads the internal, non-public ChatGPT `wham/usage` JSON endpoint. Its
availability is subject to upstream changes. The `opencode-go` slot reads the
official Console `GET /api/v1/usage/export` CSV API using a service-account
bearer key, `scope=organization`, and `range=7d`. This export is workspace-wide
accounting across model providers, not a Go quota. Go-only usage cannot be
isolated from documented CSV fields, so the public account and window labels
make that scope explicit and quota percentages and resets remain absent.
The CSV reader caps a refresh at 16 MiB; an oversized export marks only this
provider unavailable while retaining any previous successful value.
