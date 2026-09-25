#![forbid(unsafe_code)]

//! Binary entry point for the aetherd daemon.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

const HELP: &str = "\
aetherd - Linux system telemetry daemon

Usage: aetherd [OPTIONS]

Options:
      --config <PATH>  Load configuration from PATH (default: aetherd.toml if present)
  -h, --help           Print help
  -V, --version        Print version

Configuration is layered: built-in defaults, then an optional TOML file, then
AETHERD_-prefixed environment variables (nested keys use a double underscore).";

#[tokio::main]
async fn main() -> ExitCode {
    match cli::Cli::from_env() {
        Ok(cli::Cli::Help) => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(cli::Cli::Version) => {
            println!("aetherd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}\n\n{HELP}");
            ExitCode::FAILURE
        }
        Ok(cli::Cli::Run { config }) => {
            init_tracing();
            match run(config).await {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    tracing::error!(error = %error, "startup failed");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn run(config_path: Option<PathBuf>) -> Result<(), StartupError> {
    let aetherd::Config {
        http,
        paths,
        sampling,
    } = aetherd::load_config(config_path.as_deref())?;

    tracing::info!(
        bind = %http.bind,
        proc = %paths.proc.display(),
        sys = %paths.sys.display(),
        host_root = %paths.host_root.display(),
        sampling_interval_ms = sampling.interval_ms,
        "configuration loaded"
    );

    let state = aetherd::AppState::with_sampling(paths.into(), sampling);
    let app = aetherd::build_router(state.clone());
    let sampler = aetherd::spawn_sampler(&state);

    let listener = TcpListener::bind(http.bind)
        .await
        .map_err(|source| StartupError::Bind {
            addr: http.bind,
            source,
        })?;

    tracing::info!(address = %http.bind, "aetherd listening");

    let shutdown_state = state.clone();
    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            shutdown_state.shutdown();
        })
        .await;

    state.shutdown();
    if let Err(error) = sampler.await {
        tracing::warn!(error = %error, "sampler task failed");
    }

    serve_result.map_err(StartupError::Serve)?;

    tracing::info!("aetherd stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::warn!(error = %error, "SIGTERM handler unavailable");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

/// Failures that prevent the daemon from starting or serving.
#[derive(Debug, thiserror::Error)]
enum StartupError {
    #[error("configuration error")]
    Config(#[from] aetherd::ConfigError),
    #[error("failed to bind {addr}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("HTTP server failed")]
    Serve(#[source] std::io::Error),
}

mod cli {
    use std::path::PathBuf;

    pub(super) enum Cli {
        Run { config: Option<PathBuf> },
        Help,
        Version,
    }

    impl Cli {
        pub(super) fn from_env() -> Result<Self, String> {
            Self::parse(std::env::args().skip(1))
        }

        fn parse<I>(args: I) -> Result<Self, String>
        where
            I: IntoIterator<Item = String>,
        {
            let mut args = args.into_iter();
            let mut config = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "-h" | "--help" => return Ok(Self::Help),
                    "-V" | "--version" => return Ok(Self::Version),
                    "--config" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "--config requires a path".to_owned())?;
                        config = Some(PathBuf::from(value));
                    }
                    other if other.starts_with("--config=") => {
                        let value = &other["--config=".len()..];
                        if value.is_empty() {
                            return Err("--config requires a path".to_owned());
                        }
                        config = Some(PathBuf::from(value));
                    }
                    other => return Err(format!("unrecognized argument: {other}")),
                }
            }

            Ok(Self::Run { config })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::Cli;

        fn parse(args: &[&str]) -> Result<Cli, String> {
            Cli::parse(args.iter().map(|arg| (*arg).to_owned()))
        }

        #[test]
        fn no_arguments_runs_without_config() {
            assert!(matches!(parse(&[]), Ok(Cli::Run { config: None })));
        }

        #[test]
        fn config_flag_accepts_separate_value() {
            assert!(matches!(
                parse(&["--config", "custom.toml"]),
                Ok(Cli::Run { config: Some(_) })
            ));
        }

        #[test]
        fn config_flag_accepts_equals_form() {
            assert!(matches!(
                parse(&["--config=custom.toml"]),
                Ok(Cli::Run { config: Some(_) })
            ));
        }

        #[test]
        fn config_flag_requires_a_value() {
            assert!(parse(&["--config"]).is_err());
            assert!(parse(&["--config="]).is_err());
        }

        #[test]
        fn help_and_version_short_circuit() {
            assert!(matches!(parse(&["--help"]), Ok(Cli::Help)));
            assert!(matches!(parse(&["-V"]), Ok(Cli::Version)));
        }

        #[test]
        fn unknown_argument_is_rejected() {
            assert!(parse(&["--nope"]).is_err());
        }
    }
}
