#![forbid(unsafe_code)]

//! Binary entry point for the aetherd daemon.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[path = "main/cli.rs"]
mod cli;

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
        tailscale,
        providers,
    } = aetherd::load_config(config_path.as_deref())?;

    tracing::info!(
        bind = %http.bind,
        proc = %paths.proc.display(),
        sys = %paths.sys.display(),
        host_root = %paths.host_root.display(),
        sampling_interval_ms = sampling.interval_ms,
        "configuration loaded"
    );

    log_tailscale_posture(&tailscale);

    let state = aetherd::AppState::with_sampling(paths.into(), sampling)
        .with_tailscale(tailscale)
        .with_providers(providers);
    let app = aetherd::build_router(state.clone());
    let sampler = aetherd::spawn_sampler(&state);
    let tailscale_refresher = aetherd::spawn_tailscale_refresher(&state);
    let provider_refreshers = aetherd::spawn_provider_refreshers(&state);

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
    if let Some(refresher) = tailscale_refresher
        && let Err(error) = refresher.await
    {
        tracing::warn!(error = %error, "tailscale refresher task failed");
    }
    for refresher in provider_refreshers {
        if let Err(error) = refresher.await {
            tracing::warn!(error = %error, "provider refresher task failed");
        }
    }

    serve_result.map_err(StartupError::Serve)?;

    tracing::info!("aetherd stopped");
    Ok(())
}

fn log_tailscale_posture(tailscale: &aetherd::TailscaleConfig) {
    match tailscale.unavailable() {
        None => tracing::info!(
            tailnet = %tailscale.tailnet,
            refresh_interval_seconds = tailscale.refresh_interval_seconds,
            "tailscale telemetry enabled"
        ),
        Some(aetherd::TailscaleUnavailable::Disabled) => {
            tracing::debug!("tailscale telemetry disabled");
        }
        Some(reason) => tracing::warn!(reason = reason.reason(), "tailscale telemetry incomplete"),
    }
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
