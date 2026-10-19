//! Extractors that reject with problem details, and conditional-request
//! helpers (`ETag` / `If-Match`, NFR-12).

use axum::{
    extract::{FromRequest, FromRequestParts},
    http::{HeaderMap, HeaderValue, StatusCode, header},
};

use crate::problem::Problem;

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
