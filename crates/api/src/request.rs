//! Extractors that reject with problem details, and conditional-request
//! helpers (`ETag` / `If-Match`, NFR-12).

use axum::{
    extract::{FromRequest, FromRequestParts},
    http::{HeaderMap, HeaderValue, StatusCode, header},
};

use lodestar_core::ports::PageRequest;
use serde::{Deserialize, Deserializer};

use crate::problem::Problem;

/// Page size when `limit` is not given.
pub const DEFAULT_LIMIT: u32 = 50;
/// The largest `limit` a list accepts.
pub const MAX_LIMIT: u32 = 1000;

/// The page a list asks for: `limit` defaults to [`DEFAULT_LIMIT`] and must be
/// 1–[`MAX_LIMIT`]; `offset` defaults to 0.
pub fn page_request(limit: Option<u32>, offset: Option<u32>) -> Result<PageRequest, Problem> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(Problem::invalid(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    Ok(PageRequest {
        offset: offset.unwrap_or(0),
        limit,
    })
}

/// A fallback route answering 405 with the given `Allow` header.
pub fn not_allowed(allow: &'static str) -> impl Fn() -> std::future::Ready<Problem> + Clone {
    move || std::future::ready(Problem::method_not_allowed(allow))
}

/// Distinguishes a field sent as `null` (`Some(None)`) from a missing one (`None`).
pub fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::deserialize(d).map(Some)
}

/// A JSON body; malformed or invalid bodies are problem details.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(Problem))]
pub struct JsonBody<T>(pub T);

/// A query string; invalid parameters are a 422 problem.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(Problem))]
pub struct Query<T>(pub T);

/// A path parameter; malformed values are a 400 problem.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(Problem))]
pub struct Path<T>(pub T);

/// The strong `ETag` for a resource version.
pub fn etag(version: u32) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{version}\"")).expect("digits are a valid header value")
}

/// The version a write is based on, from `If-Match`: 428 if missing, 412 if
/// it names no version (it cannot match).
pub fn if_match(headers: &HeaderMap) -> Result<u32, Problem> {
    let value = headers.get(header::IF_MATCH).ok_or_else(|| {
        Problem::new(StatusCode::PRECONDITION_REQUIRED)
            .detail("send If-Match with the ETag of the version being changed")
    })?;
    value
        .to_str()
        .ok()
        .map(str::trim)
        .and_then(|v| v.strip_prefix('"')?.strip_suffix('"'))
        .and_then(|v| v.parse().ok())
        .ok_or_else(Problem::stale)
}
