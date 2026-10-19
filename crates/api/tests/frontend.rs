use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use rust_embed::RustEmbed;
use tower::ServiceExt;

struct AlwaysHealthy;

impl lodestar_core::health::HealthProbe for AlwaysHealthy {
    async fn database_reachable(&self) -> bool {
        true
    }
}

#[derive(RustEmbed)]
#[folder = "tests/fixtures/dist"]
struct BuiltFrontend;

#[derive(RustEmbed)]
#[folder = "tests/fixtures/missing"]
#[allow_missing = true]
struct MissingFrontend;

async fn get(app: Router, path: &str) -> Response {
    app.oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body_text(response: Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn header_value<'a>(response: &'a Response, name: &str) -> &'a str {
    response
        .headers()
        .get(name)
        .unwrap_or_else(|| panic!("missing header {name}"))
        .to_str()
        .unwrap()
}

#[tokio::test]
async fn root_serves_index_html_without_caching() {
    let response = get(
        lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy),
        "/",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_value(&response, "cache-control"), "no-cache");
    assert!(header_value(&response, "content-type").starts_with("text/html"));
    assert!(body_text(response).await.contains(r#"<div id="app">"#));
}

#[tokio::test]
async fn hashed_assets_are_cached_immutably() {
    let response = get(
        lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy),
        "/assets/index-abc123.js",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_value(&response, "cache-control"),
        "public, max-age=31536000, immutable"
    );
    assert!(header_value(&response, "content-type").contains("javascript"));
}

#[tokio::test]
async fn extensionless_client_routes_fall_back_to_index_html() {
    let response = get(
        lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy),
        "/projects/abc",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_value(&response, "cache-control"), "no-cache");
    assert!(body_text(response).await.contains(r#"<div id="app">"#));
}

#[tokio::test]
async fn missing_files_with_an_extension_are_not_found() {
    let response = get(
        lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy),
        "/assets/nope.js",
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn api_paths_do_not_fall_back_to_index_html() {
    let response = get(
        lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy),
        "/api/v1/unknown",
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(!body_text(response).await.contains("<div"));
}

#[tokio::test]
async fn root_says_frontend_not_built_when_dist_is_missing() {
    let response = get(
        lodestar_api::router_with::<MissingFrontend, _>(AlwaysHealthy),
        "/",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(header_value(&response, "content-type").starts_with("text/plain"));
    assert!(body_text(response).await.contains("frontend not built"));
}

#[tokio::test]
async fn every_response_carries_security_headers() {
    for path in ["/", "/assets/index-abc123.js", "/api/v1/unknown", "/x.png"] {
        let response = get(
            lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy),
            path,
        )
        .await;
        let csp = header_value(&response, "content-security-policy");
        assert!(csp.contains("default-src 'self'"), "{path}: {csp}");
        assert!(csp.contains("script-src 'self'"), "{path}: {csp}");
        assert!(!csp.contains("unsafe-inline"), "{path}: {csp}");
        assert!(csp.contains("frame-ancestors 'none'"), "{path}: {csp}");
        assert_eq!(header_value(&response, "x-content-type-options"), "nosniff");
        assert_eq!(header_value(&response, "x-frame-options"), "DENY");
        assert_eq!(header_value(&response, "referrer-policy"), "no-referrer");
    }
}

#[tokio::test]
async fn responses_are_compressed_when_the_client_accepts_it() {
    let response = lodestar_api::router_with::<BuiltFrontend, _>(AlwaysHealthy)
        .oneshot(
            Request::get("/")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(header_value(&response, "content-encoding"), "gzip");
}
