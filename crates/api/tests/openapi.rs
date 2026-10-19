mod support;

use axum::http::StatusCode;

/// Regenerate with `just contract` (NFR-62, ADR-0002).
#[test]
fn committed_openapi_json_matches_generation() {
    let committed = include_str!("../../../openapi/openapi.json");
    assert!(
        committed == lodestar_api::openapi_json(),
        "openapi/openapi.json is stale; run `just contract`"
    );
}

#[test]
fn read_only_fields_are_marked_on_the_single_person_schema() {
    let doc: serde_json::Value = serde_json::from_str(&lodestar_api::openapi_json()).unwrap();
    let person = &doc["components"]["schemas"]["Person"]["properties"];
    assert_eq!(person["id"]["readOnly"], true);
    assert_eq!(person["version"]["readOnly"], true);
    assert!(person["display_name"].get("readOnly").is_none());
}

#[tokio::test]
async fn the_app_serves_its_openapi_document() {
    let app = support::app().await;
    let reply = app.get("/api/v1/openapi.json", "").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.body["paths"]["/api/v1/people"].is_object());
}

#[tokio::test]
async fn unknown_api_paths_are_a_404_problem() {
    let app = support::app().await;
    let reply = app.get("/api/v1/nothing-here", "").await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.header("content-type"), "application/problem+json");
}
