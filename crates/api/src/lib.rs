//! HTTP layer: axum handlers and the embedded frontend.

mod frontend;
mod security;

use axum::{Router, http::StatusCode, routing::any};
use rust_embed::RustEmbed;
use tower_http::compression::CompressionLayer;

/// The Vite build output, embedded at compile time. Empty when `frontend/dist`
/// has not been built, in which case `/` explains that.
#[derive(RustEmbed)]
#[folder = "../../frontend/dist"]
#[allow_missing = true]
struct Frontend;

/// The application router, serving the embedded frontend.
pub fn router() -> Router {
    router_with::<Frontend>()
}

/// The application router, serving the frontend from the given asset set.
pub fn router_with<A: RustEmbed + Send + Sync + 'static>() -> Router {
    let api = Router::new().fallback(any(|| async { StatusCode::NOT_FOUND }));

    let app = Router::new()
        .nest("/api", api)
        .fallback(frontend::serve::<A>)
        .layer(CompressionLayer::new());
    security::with_headers(app)
}
