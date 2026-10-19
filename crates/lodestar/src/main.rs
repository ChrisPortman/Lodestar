//! The Lodestar binary: config, wiring and startup.

mod cli;
mod config;
mod logging;

use std::{net::SocketAddr, process::ExitCode};

use cli::Command;
use config::KeySource;
use lodestar_datarepo::sqlite::SqliteDatabase;

/// sysexits EX_USAGE.
const EX_USAGE: u8 = 64;
/// sysexits EX_CONFIG.
const EX_CONFIG: u8 = 78;

#[tokio::main]
async fn main() -> ExitCode {
    let config_path = match cli::parse(std::env::args_os().skip(1)) {
        Ok(Command::Run { config }) => config,
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

    let config = match config::load(config_path.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "invalid configuration");
            return ExitCode::from(EX_CONFIG);
        }
    };
    logging.apply(&config.log);
    tracing::info!(config = ?config, "configuration loaded");
    if !config.server.bind.is_loopback() {
        tracing::warn!(
            bind = %config.server.bind,
            "server.bind is not a loopback address; Lodestar has no authentication yet (NFR-30)"
        );
    }

    let _session_key = match config::session_key(&config.session) {
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
    tracing::info!("Lodestar listening on http://{addr}");
    if let Err(error) = axum::serve(listener, lodestar_api::router(database)).await {
        tracing::error!(%error, "server error");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
