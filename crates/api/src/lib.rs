//! HTTP layer: axum handlers and the embedded frontend.

mod actions;
mod blockers;
mod export;
mod filters;
mod frontend;
mod health;
mod members;
mod people;
mod problem;
mod projects;
mod request;
mod risks;
mod security;
mod services;
mod session;

use std::sync::Arc;

use axum::{Json, Router, http::StatusCode, routing::get};
use axum_extra::extract::cookie::Key;
use lodestar_core::{
    health::HealthProbe,
    ports::{
        ActionsQuery, BlockersQuery, HistoryQuery, PeopleQuery, ProjectsQuery, RisksQuery,
        UnitOfWork,
    },
};
use rust_embed::RustEmbed;
use tower_http::compression::CompressionLayer;
use utoipa::OpenApi;

pub use problem::Problem;

/// The Vite build output, embedded at compile time. Empty when `frontend/dist`
/// has not been built, in which case `/` explains that.
#[derive(RustEmbed)]
#[folder = "../../frontend/dist"]
#[allow_missing = true]
struct Frontend;

/// What the router needs from the rest of the application.
pub struct Services<H, S> {
    pub health: H,
    /// Every repository and query port.
    pub store: S,
    /// Signs session cookies; at least 64 bytes.
    pub session_key: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub people: Arc<dyn services::People>,
    pub projects: Arc<dyn services::Projects>,
    pub risks: Arc<dyn services::Risks>,
    pub actions: Arc<dyn services::Actions>,
    pub blockers: Arc<dyn services::Blockers>,
    pub history: Arc<dyn export::ProjectHistory>,
    pub session_key: Key,
}

/// The application router, serving the embedded frontend.
pub fn router<H, S>(services: Services<H, S>) -> Router
where
    H: HealthProbe,
    S: UnitOfWork
        + PeopleQuery
        + ProjectsQuery
        + RisksQuery
        + ActionsQuery
        + BlockersQuery
        + HistoryQuery
        + Clone
        + 'static,
{
    router_with::<Frontend, H, S>(services)
}

/// The application router, serving the frontend from the given asset set.
///
/// # Panics
/// If the session key is shorter than 64 bytes (config validation prevents it).
pub fn router_with<A, H, S>(services: Services<H, S>) -> Router
where
    A: RustEmbed + Send + Sync + 'static,
    H: HealthProbe,
    S: UnitOfWork
        + PeopleQuery
        + ProjectsQuery
        + RisksQuery
        + ActionsQuery
        + BlockersQuery
        + HistoryQuery
        + Clone
        + 'static,
{
    let state = AppState {
        people: Arc::new(services.store.clone()),
        projects: Arc::new(services.store.clone()),
        risks: Arc::new(services.store.clone()),
        actions: Arc::new(services.store.clone()),
        blockers: Arc::new(services.store.clone()),
        history: Arc::new(services.store),
        session_key: Key::try_from(services.session_key.as_slice())
            .expect("session key must be at least 64 bytes"),
    };
    let v1 = Router::new()
        .merge(people::routes())
        .merge(projects::routes())
        .merge(risks::routes())
        .merge(actions::routes())
        .merge(blockers::routes())
        .merge(members::routes())
        .merge(export::routes())
        .merge(session::routes())
        .route("/openapi.json", get(|| async { Json(openapi()) }))
        .with_state(state);
    let api = Router::new()
        .nest("/v1", v1)
        .fallback(|| async { Problem::new(StatusCode::NOT_FOUND) });

    let app = Router::new()
        .nest("/api", api)
        .merge(health::routes(services.health))
        .fallback(frontend::serve::<A>)
        .layer(CompressionLayer::new());
    security::with_headers(app)
}

#[derive(OpenApi)]
#[openapi(
    info(title = "Lodestar API", description = "Lodestar REST API (ADR-0006)."),
    paths(
        people::list,
        people::create,
        people::fetch,
        people::update,
        projects::list,
        projects::suggest_code,
        projects::create,
        projects::fetch,
        projects::update,
        projects::archive,
        risks::list,
        risks::create,
        risks::fetch,
        risks::update,
        risks::close,
        actions::list,
        actions::create,
        actions::fetch,
        actions::update,
        actions::close,
        blockers::list,
        blockers::create,
        blockers::fetch,
        blockers::update,
        blockers::clear,
        blockers::link,
        blockers::unlink,
        members::list,
        members::add,
        members::remove,
        export::export,
        session::current,
        session::choose,
    )
)]
struct ApiDoc;

/// The OpenAPI document for `/api/v1` (ADR-0002).
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    // Fixed so the committed document does not change with each release.
    doc.info.version = "1".to_owned();
    doc.info.license = None;
    doc
}

/// The OpenAPI document as committed to `openapi/openapi.json`.
pub fn openapi_json() -> String {
    let mut json = openapi()
        .to_pretty_json()
        .expect("the OpenAPI document serialises");
    json.push('\n');
    json
}
