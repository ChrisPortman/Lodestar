//! `GET /healthz`: the process is running and the database is reachable.

use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use lodestar_core::health::HealthProbe;
use serde_json::{Value, json};

pub fn routes<H: HealthProbe>(probe: H) -> Router {
    Router::new()
        .route("/healthz", get(healthz::<H>))
        .with_state(Arc::new(probe))
}

async fn healthz<H: HealthProbe>(State(probe): State<Arc<H>>) -> (StatusCode, Json<Value>) {
    if probe.database_reachable().await {
        (
            StatusCode::OK,
            Json(json!({"status": "ok", "database": "reachable"})),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "degraded", "database": "unreachable"})),
        )
    }
}
