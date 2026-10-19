//! Serving the embedded single-page app, with SPA fallback and cache headers.

use axum::{
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

const INDEX: &str = "index.html";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";

pub(crate) async fn serve<A: RustEmbed>(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if !path.is_empty()
        && let Some(response) = asset::<A>(path)
    {
        return response;
    }

    let is_client_route = !path.rsplit('/').next().unwrap_or("").contains('.');
    if !is_client_route {
        return StatusCode::NOT_FOUND.into_response();
    }

    asset::<A>(INDEX).unwrap_or_else(|| {
        (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Lodestar: frontend not built. Run `just frontend` and rebuild.\n",
        )
            .into_response()
    })
}

fn asset<A: RustEmbed>(path: &str) -> Option<Response> {
    let file = A::get(path)?;
    let cache = if path.starts_with("assets/") {
        IMMUTABLE
    } else {
        NO_CACHE
    };
    let etag = format!("\"{}\"", hex(&file.metadata.sha256_hash()));
    let mime = file.metadata.mimetype().to_owned();
    Some(
        (
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, cache.to_owned()),
                (header::ETAG, etag),
            ],
            file.data,
        )
            .into_response(),
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
