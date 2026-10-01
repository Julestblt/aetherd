# Configuration

Configuration is layered, later sources winning:

1. Built-in defaults.
2. An optional TOML file.
3. `AETHERD_`-prefixed environment variables.

The file is optional. Defaults plus environment variables are enough to run,
which is the intended path for containers.

## Sources

- File: `--config <PATH>`, or `aetherd.toml` in the working directory when it
  exists. An explicitly requested file that does not exist is an error.
- Environment: `AETHERD_` prefix, with `__` separating nested keys. For example
  `AETHERD_HTTP__BIND` sets `http.bind`.

Unknown keys are rejected, including unknown `AETHERD_` variables, so a typo
fails loudly instead of being ignored.

## Options

| Key | Environment | Default | Description |
|---|---|---|---|
| `http.bind` | `AETHERD_HTTP__BIND` | `127.0.0.1:8080` | HTTP listen address. Use `0.0.0.0:8080` in a container. |
| `paths.proc` | `AETHERD_PATHS__PROC` | `/proc` | procfs root. |
| `paths.sys` | `AETHERD_PATHS__SYS` | `/sys` | sysfs root. |
| `paths.host_root` | `AETHERD_PATHS__HOST_ROOT` | `/` | Host filesystem root. |
| `sampling.interval_ms` | `AETHERD_SAMPLING__INTERVAL_MS` | `1000` | Background sampling interval, in milliseconds. |
| `tailscale.enabled` | `AETHERD_TAILSCALE__ENABLED` | `false` | Collect tailnet machines from the Tailscale HTTP API. |
| `tailscale.tailnet` | `AETHERD_TAILSCALE__TAILNET` | `""` | Tailnet to query, for example `example.ts.net`. |
| `tailscale.api_key` | `AETHERD_TAILSCALE__API_KEY` | `""` | Tailscale API key. Never serialized or logged. |
| `tailscale.refresh_interval_seconds` | `AETHERD_TAILSCALE__REFRESH_INTERVAL_SECONDS` | `60` | Tailscale refresh interval, in seconds (`1..=86400`). |
| `providers.codex.enabled` | `AETHERD_PROVIDERS__CODEX__ENABLED` | `false` | Enable Codex account quota. |
| `providers.codex.auth_file` | `AETHERD_PROVIDERS__CODEX__AUTH_FILE` | unset | Explicit Codex auth file path. |
| `providers.codex.refresh_interval_seconds` | `AETHERD_PROVIDERS__CODEX__REFRESH_INTERVAL_SECONDS` | `60` | Codex refresh interval in seconds (`1..=86400`). |
| `providers.opencode.enabled` | `AETHERD_PROVIDERS__OPENCODE__ENABLED` | `false` | Enable OpenCode Go account quota. |
| `providers.opencode.auth_file` | `AETHERD_PROVIDERS__OPENCODE__AUTH_FILE` | unset | Explicit OpenCode auth file path. |
| `providers.opencode.refresh_interval_seconds` | `AETHERD_PROVIDERS__OPENCODE__REFRESH_INTERVAL_SECONDS` | `60` | OpenCode Go refresh interval in seconds (`1..=86400`). |

## AI usage providers

Codex resolves its auth file in this exact order:

1. `AETHERD_PROVIDERS__CODEX__AUTH_FILE` (or the equivalent TOML field)
2. `$CODEX_HOME/auth.json`
3. `$HOME/.codex/auth.json`

Only the resolved file is read. An explicit path that is missing is reported as
unavailable; the daemon does not silently switch accounts. The file must hold
a Codex OAuth `tokens.access_token`. API-key-only Codex auth is unsupported.
No Codex CLI is needed in the container. The OAuth access token is reread on
every refresh, so file updates become effective without restarting aetherd.
The provider does not refresh expired OAuth tokens itself.

Local development with the default auth file:

```bash
export AETHERD_PROVIDERS__CODEX__ENABLED=true
cargo run
```

An explicit local file uses
`AETHERD_PROVIDERS__CODEX__AUTH_FILE="$HOME/.codex/auth.json"`.
For Dokploy or Docker, bind mount the host's `~/.codex/auth.json` to
`/run/secrets/codex-auth.json` read-only, then set:

```text
AETHERD_PROVIDERS__CODEX__ENABLED=true
AETHERD_PROVIDERS__CODEX__AUTH_FILE=/run/secrets/codex-auth.json
```

The non-root container user must be able to read the mounted file. The Codex
usage URL is an internal ChatGPT endpoint, not a stable public API; upstream
authentication and response fields can change without notice.

OpenCode Go uses the `opencode-go` API key entry in OpenCode's structured
`auth.json`. It resolves an explicit `providers.opencode.auth_file`, then
`$XDG_DATA_HOME/opencode/auth.json`, then
`$HOME/.local/share/opencode/auth.json`. The ordinary `opencode` Zen key is not
accepted for Go quota. For a container, mount the OpenCode auth file read-only
and set `AETHERD_PROVIDERS__OPENCODE__AUTH_FILE` to its container path.
The Go usage route exists in OpenCode's official source, but its stability is
not guaranteed by a published API contract.

```toml
[providers.codex]
enabled = false
refresh_interval_seconds = 60

[providers.opencode]
enabled = false
refresh_interval_seconds = 60
```

Log verbosity is controlled by the standard `RUST_LOG` environment variable
(for example `RUST_LOG=info,aetherd=debug`).

## Sampling

One background task samples the whole system on `sampling.interval_ms` and keeps
the latest snapshot in memory for REST and the SSE stream. The interval must be
between `100` and `3600000` milliseconds; any other value is rejected at
startup with a clear error. Shorter intervals give fresher values and more CPU
and I/O, so the default of one second is a reasonable balance.

## Tailscale

Tailscale telemetry is optional and reads the official HTTP API; no `tailscale`
CLI or host socket is needed. It is off by default. Enable it by setting
`tailscale.enabled`, a tailnet, and an API key. If any of the three is missing,
aetherd still starts, every other metric keeps working, and the Tailscale
section reports why it is unavailable.

`tailscale.refresh_interval_seconds` is independent of `sampling.interval_ms`.
Device state does not change second to second, so the API is not polled once per
sample; the system snapshot keeps a cached Tailscale section that refreshes on
its own schedule:

```text
system sampling:      1000 ms
tailscale refresh:       60 s
```

The interval must be between `1` and `86400` seconds; any other value is
rejected at startup. Provide the API key through the environment (or a
secrets-manager-injected env file) rather than a committed TOML file.

## TOML example

```toml
[http]
bind = "0.0.0.0:8080"

[paths]
proc = "/host/proc"
sys = "/host/sys"
host_root = "/host/root"

[sampling]
interval_ms = 1000

[tailscale]
enabled = false
tailnet = ""
refresh_interval_seconds = 60
```

## Container example

The `compose.yaml` in this repository sets exactly these variables and mounts
the host paths read-only:

```yaml
environment:
  AETHERD_HTTP__BIND: 0.0.0.0:8080
  AETHERD_PATHS__PROC: /host/proc
  AETHERD_PATHS__SYS: /host/sys
  AETHERD_PATHS__HOST_ROOT: /host/root
  AETHERD_TAILSCALE__ENABLED: "true"
  AETHERD_TAILSCALE__TAILNET: example.ts.net
  AETHERD_TAILSCALE__API_KEY: ${TAILSCALE_API_KEY}
  AETHERD_TAILSCALE__REFRESH_INTERVAL_SECONDS: "60"
```

The Tailscale variables are only needed when the integration is enabled; supply
the API key from your platform's secret store instead of writing it into the
compose file.

## Secrets

Credentials are read from the environment only and represented by a redacting
`SecretString` type, so they cannot be logged, serialized, or returned by an
endpoint by accident. The Tailscale API key is the first credential of this
kind; AI provider credentials will follow the same rule. Do not commit
credentials: inject them through the environment, a secrets manager, or an
untracked env file.
