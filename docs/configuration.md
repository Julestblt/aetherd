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

Log verbosity is controlled by the standard `RUST_LOG` environment variable
(for example `RUST_LOG=info,aetherd=debug`).

## TOML example

```toml
[http]
bind = "0.0.0.0:8080"

[paths]
proc = "/host/proc"
sys = "/host/sys"
host_root = "/host/root"
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
```

## Secrets

Provider credentials are not part of configuration yet. When AI providers land,
credentials will be read from the environment only, represented by a redacting
type so they cannot be logged, and never committed.
