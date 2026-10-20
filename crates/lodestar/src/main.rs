//! The Lodestar binary: config, wiring and startup.

mod cli;
mod config;
mod local;
mod logging;

use std::{net::SocketAddr, path::Path, process::ExitCode};

use cli::Command;
use config::{Config, ConfigError, KeySource};
use local::{Claim, Instance};

/// A loaded config and the settings `--local` ignored.
type Loaded = (Config, Vec<&'static str>);
use lodestar_datarepo::sqlite::SqliteDatabase;

/// sysexits EX_USAGE.
const EX_USAGE: u8 = 64;
/// sysexits EX_CONFIG.
const EX_CONFIG: u8 = 78;

#[tokio::main]
async fn main() -> ExitCode {
    let (config_path, local) = match cli::parse(std::env::args_os().skip(1)) {
        Ok(Command::Run { config, local }) => (config, local),
        Ok(Command::OpenApi { out }) => return write_openapi(&out),
        Ok(Command::Help) => {
            print!("{}", cli::HELP);
            return ExitCode::SUCCESS;
        }
        Ok(Command::Version) => {
            println!("lodestar {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("lodestar: {message}\n\n{}", cli::HELP);
            return ExitCode::from(EX_USAGE);
        }
    };

    let logging = logging::init();

    let (instance, loaded) = if local {
        match claim_local(config_path.as_deref()) {
            Ok(started) => started,
            Err(code) => return code,
        }
    } else {
        let loaded = config::load(config_path.as_deref()).map(|config| (config, Vec::new()));
        (None, loaded)
    };
    let (config, ignored) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            tracing::error!(%error, "invalid configuration");
            return ExitCode::from(EX_CONFIG);
        }
    };
    logging.apply(&config.log);
    for key in ignored {
        tracing::warn!(key, "ignoring this setting: --local decides it");
    }
    tracing::info!(config = ?config, "configuration loaded");
    if !config.server.bind.is_loopback() {
        tracing::warn!(
            bind = %config.server.bind,
            "server.bind is not a loopback address; Lodestar has no authentication yet (NFR-30)"
        );
    }

    let session_key = match config::session_key(&config.session) {
        Ok((key, source)) => {
            match source {
                KeySource::Generated(path) => {
                    tracing::info!(path = %path.display(), "generated new session key")
                }
                KeySource::File(path) => {
                    tracing::debug!(path = %path.display(), "loaded session key")
                }
                KeySource::Config => tracing::debug!("using session key from config"),
            }
            key
        }
        Err(error) => {
            tracing::error!(%error, "invalid configuration");
            return ExitCode::from(EX_CONFIG);
        }
    };

    let database = match SqliteDatabase::connect(&config.database.url).await {
        Ok(database) => database,
        Err(error) => {
            tracing::error!(%error, url = %config.database.url, "cannot open database");
            return ExitCode::from(EX_CONFIG);
        }
    };

    let addr = SocketAddr::new(config.server.bind, config.server.port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%error, %addr, "cannot bind");
            return ExitCode::FAILURE;
        }
    };
    let addr = listener.local_addr().unwrap_or(addr);
    tracing::info!("Lodestar listening on http://{addr}");
    if let Some(instance) = &instance {
        if let Err(error) = instance.record_port(addr.port()) {
            tracing::warn!(%error, "cannot record the port; a second --local launch won't find this one");
        }
        local::open(addr.port());
    }
    let served = axum::serve(
        listener,
        lodestar_api::router(lodestar_api::Services {
            store: database.store(),
            health: database.clone(),
            session_key,
        }),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    database.close().await;
    if let Err(error) = served {
        tracing::error!(%error, "server error");
        return ExitCode::FAILURE;
    }
    tracing::info!("Lodestar stopped");
    ExitCode::SUCCESS
}

/// The local-mode lock and config, or the exit code once the launch is
/// finished: a running instance was opened instead, or the data directory
/// couldn't be claimed.
fn claim_local(
    config_path: Option<&Path>,
) -> Result<(Option<Instance>, Result<Loaded, ConfigError>), ExitCode> {
    let data_dir = local::data_dir().map_err(|error| {
        tracing::error!(%error, "cannot create the data directory");
        ExitCode::FAILURE
    })?;
    match local::claim(&data_dir) {
        Ok(Claim::Acquired(instance)) => {
            Ok((Some(instance), config::load_local(config_path, &data_dir)))
        }
        Ok(Claim::Running(port)) => {
            local::open(port);
            Err(ExitCode::SUCCESS)
        }
        Err(error) => {
            tracing::error!(%error, dir = %data_dir.display(), "cannot claim the data directory");
            Err(ExitCode::FAILURE)
        }
    }
}

/// Resolves on SIGINT (Ctrl-C) or, on Unix, SIGTERM. The server then stops
/// accepting connections and finishes the requests in flight.
async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}

fn write_openapi(out: &std::path::Path) -> ExitCode {
    let result = out
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(out, lodestar_api::openapi_json()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lodestar: cannot write {}: {error}", out.display());
            ExitCode::FAILURE
        }
    }
}
