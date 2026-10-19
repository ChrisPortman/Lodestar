//! Security headers applied to every response (NFR-32).

use axum::{
    Router,
    http::{HeaderName, HeaderValue, header},
};
use tower_http::set_header::SetResponseHeaderLayer;

/// Same-origin only; no inline scripts, no third-party sources.
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; \
base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

const HEADERS: [(HeaderName, &str); 5] = [
    (header::CONTENT_SECURITY_POLICY, CSP),
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::X_FRAME_OPTIONS, "DENY"),
    (header::REFERRER_POLICY, "no-referrer"),
    (
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    ),
];

pub(crate) fn with_headers(router: Router) -> Router {
    HEADERS.into_iter().fold(router, |router, (name, value)| {
        router.layer(SetResponseHeaderLayer::overriding(
            name,
            HeaderValue::from_static(value),
        ))
    })
}
