//! The Lodestar binary: config, wiring and startup.

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr: SocketAddr = std::env::var("LODESTAR_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()
        .map_err(std::io::Error::other)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("Lodestar listening on http://{addr}");
    axum::serve(listener, lodestar_api::router()).await
}
