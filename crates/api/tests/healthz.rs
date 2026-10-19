use axum::{body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use lodestar_core::health::HealthProbe;
use tower::ServiceExt;

struct Database {
    reachable: bool,
}

impl HealthProbe for Database {
    async fn database_reachable(&self) -> bool {
        self.reachable
    }
}

async fn healthz(reachable: bool) -> (StatusCode, serde_json::Value) {
    let response = lodestar_api::router(lodestar_api::Services {
        health: Database { reachable },
        store: lodestar_core::test_support::InMemoryStore::new(),
        session_key: vec![0; 64],
    })
    .oneshot(Request::get("/api/v1/healthz").body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn healthz_reports_running_and_database_reachable() {
    let (status, body) = healthz(true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({"status": "ok", "database": "reachable"})
    );
}

#[tokio::test]
async fn healthz_is_unavailable_when_the_database_is_unreachable() {
    let (status, body) = healthz(false).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body,
        serde_json::json!({"status": "degraded", "database": "unreachable"})
    );
}
