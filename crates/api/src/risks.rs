//! `/api/v1/risks`: list, create, fetch, edit, close and reopen Risks
//! (FR-40 to FR-44). There is no conversion to a Blocker (FR-44).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use lodestar_core::{
    ports::{PageRequest, RiskFilter, RiskSort},
    project::ProjectId,
    risk::{
        RiskEdit, RiskId, RiskView,
        use_cases::{NewRisk, create_risk, get_risk, list_risks, set_closed, update_risk},
    },
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState, Store,
    filters::StateParam,
    problem::Problem,
    request::{JsonBody, Path, Query, etag, if_match, not_allowed, page_request},
    session::Acting,
};

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new()
        .route(
            "/risks",
            get(list::<S>)
                .post(create::<S>)
                .delete(not_allowed("GET,HEAD,POST")),
        )
        .route(
            "/risks/{id}",
            get(fetch::<S>)
                .put(update::<S>)
                .patch(close::<S>)
                .delete(not_allowed("GET,HEAD,PUT,PATCH")),
        )
}

/// A Risk. Also the request body for `POST` and `PUT`: read-only fields may
/// be sent to `PUT` only if they match what is stored.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = Risk)]
pub struct RiskBody {
    #[schema(read_only)]
    pub id: Uuid,
    /// Required on `POST`; never changes.
    pub project_id: Uuid,
    #[schema(read_only)]
    pub project_code: String,
    /// `R-001`, … numbered within the Project. A label, not an identifier.
    #[schema(read_only)]
    pub reference: String,
    /// 1–4000 characters once trimmed; may span lines.
    #[schema(min_length = 1, max_length = 4000)]
    pub description: String,
    #[schema(minimum = 1, maximum = 5)]
    pub likelihood: u8,
    #[schema(minimum = 1, maximum = 5)]
    pub impact: u8,
    /// Likelihood × impact, calculated (FR-41).
    #[schema(read_only, minimum = 1, maximum = 25)]
    pub score: u8,
    /// Changed only with `PATCH`.
    #[schema(read_only)]
    pub closed: bool,
    #[schema(read_only, minimum = 1)]
    pub version: u32,
}

impl From<RiskView> for RiskBody {
    fn from(view: RiskView) -> Self {
        Self {
            id: view.id.as_uuid(),
            project_id: view.project_id.as_uuid(),
            project_code: view.project_code,
            reference: view.reference,
            description: view.description,
            likelihood: view.likelihood,
            impact: view.impact,
            score: view.score,
            closed: view.closed,
            version: view.version,
        }
    }
}

/// What a client may send for a Risk; documented as [`RiskBody`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RiskInput {
    id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_code: Option<String>,
    reference: Option<String>,
    description: String,
    likelihood: u8,
    impact: u8,
    score: Option<u8>,
    closed: Option<bool>,
    version: Option<u32>,
}

impl RiskInput {
    fn edit(&self) -> RiskEdit<'_> {
        RiskEdit {
            description: &self.description,
            likelihood: self.likelihood,
            impact: self.impact,
        }
    }
}

/// The only change `PATCH` makes (ADR-0006).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RiskPatch {
    pub closed: bool,
}

fn with_etag(status: StatusCode, risk: RiskBody) -> Response {
    (status, [(header::ETAG, etag(risk.version))], Json(risk)).into_response()
}

/// A page of Risks.
#[derive(Debug, Serialize, ToSchema)]
pub struct RiskPage {
    pub items: Vec<RiskBody>,
    /// Matching Risks across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskSortParam {
    /// Creation order.
    #[default]
    Id,
    /// Reference number within each Project.
    Reference,
    /// Highest score first.
    Score,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Only this Project's Risks.
    project_id: Option<Uuid>,
    /// Page size, 1–1000.
    #[param(minimum = 1, maximum = 1000, default = 50)]
    limit: Option<u32>,
    #[param(minimum = 0, default = 0)]
    offset: Option<u32>,
    #[param(inline)]
    sort: Option<RiskSortParam>,
    /// Include closed Risks. Ignored when `state` is given.
    #[param(default = false)]
    include_closed: Option<bool>,
    /// Open (the default), closed or all Risks.
    #[param(inline)]
    state: Option<StateParam>,
}

/// Lists Risks, ordered by ID unless `sort` is given. Closed Risks are left
/// out unless `include_closed=true`.
#[utoipa::path(
    get,
    path = "/api/v1/risks",
    tag = "risks",
    operation_id = "list_risks",
    params(ListParams),
    responses(
        (status = 200, body = RiskPage),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn list<S: Store>(
    State(state): State<AppState<S>>,
    Query(params): Query<ListParams>,
) -> Result<Json<RiskPage>, Problem> {
    let PageRequest { offset, limit } = page_request(params.limit, params.offset)?;
    let sort = match params.sort.unwrap_or_default() {
        RiskSortParam::Id => RiskSort::Id,
        RiskSortParam::Reference => RiskSort::Reference,
        RiskSortParam::Score => RiskSort::Score,
    };
    let (include_closed, closed_only) = StateParam::flags(params.state, params.include_closed);
    let filter = RiskFilter {
        project_id: params.project_id.map(ProjectId::from_uuid),
        include_closed,
        closed_only,
    };
    let page = list_risks(&state.store, PageRequest { offset, limit }, filter, sort).await?;
    Ok(Json(RiskPage {
        items: page.items.into_iter().map(RiskBody::from).collect(),
        total: page.total,
        limit,
        offset,
    }))
}

/// Creates an open Risk with the next Reference in its Project.
#[utoipa::path(
    post,
    path = "/api/v1/risks",
    tag = "risks",
    operation_id = "create_risk",
    request_body = RiskBody,
    responses(
        (status = 201, body = RiskBody, headers(("location" = String), ("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Risk", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    JsonBody(input): JsonBody<RiskInput>,
) -> Result<Response, Problem> {
    if input.id.is_some()
        || input.project_code.is_some()
        || input.reference.is_some()
        || input.score.is_some()
        || input.closed.is_some()
        || input.version.is_some()
    {
        return Err(Problem::invalid(
            "id, project_code, reference, score, closed and version are set by the server",
        ));
    }
    let Some(project_id) = input.project_id else {
        return Err(Problem::invalid("project_id is required"));
    };
    let edit = input.edit();
    let risk = create_risk(
        &state.store,
        acting,
        NewRisk {
            project_id: ProjectId::from_uuid(project_id),
            description: edit.description,
            likelihood: edit.likelihood,
            impact: edit.impact,
        },
    )
    .await?;
    let body = RiskBody::from(get_risk(&state.store, risk.id()).await?);
    let location = format!("/api/v1/risks/{}", body.id);
    Ok((
        [(header::LOCATION, location)],
        with_etag(StatusCode::CREATED, body),
    )
        .into_response())
}

/// Fetches a Risk, open or closed.
#[utoipa::path(
    get,
    path = "/api/v1/risks/{id}",
    tag = "risks",
    operation_id = "get_risk",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = RiskBody, headers(("etag" = String))),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
async fn fetch<S: Store>(
    State(state): State<AppState<S>>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    let view = get_risk(&state.store, RiskId::from_uuid(id)).await?;
    Ok(with_etag(StatusCode::OK, view.into()))
}

/// Replaces a Risk's description, likelihood and impact. Needs a session and
/// `If-Match`.
#[utoipa::path(
    put,
    path = "/api/v1/risks/{id}",
    tag = "risks",
    operation_id = "update_risk",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body = RiskBody,
    responses(
        (status = 200, body = RiskBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Risk", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn update<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<RiskInput>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if input.version.is_some_and(|v| v != expected) {
        return Err(Problem::stale());
    }
    if input.id.is_some_and(|body_id| body_id != id) {
        return Err(Problem::invalid("id does not match the URL"));
    }
    let id = RiskId::from_uuid(id);
    let stored = get_risk(&state.store, id).await?;
    let mismatch = input
        .project_id
        .is_some_and(|p| p != stored.project_id.as_uuid())
        || input
            .project_code
            .as_ref()
            .is_some_and(|c| *c != stored.project_code)
        || input
            .reference
            .as_ref()
            .is_some_and(|r| *r != stored.reference)
        || input.closed.is_some_and(|c| c != stored.closed);
    if mismatch {
        return Err(Problem::invalid(
            "project_id, project_code, reference and closed cannot be changed with PUT",
        ));
    }
    // A stale score is ignored rather than rejected: it follows from the ratings.
    update_risk(&state.store, acting, id, expected, input.edit()).await?;
    Ok(with_etag(
        StatusCode::OK,
        get_risk(&state.store, id).await?.into(),
    ))
}

/// Closes or reopens a Risk, with `{"closed": …}` as
/// `application/merge-patch+json`. Needs a session and `If-Match`.
#[utoipa::path(
    patch,
    path = "/api/v1/risks/{id}",
    tag = "risks",
    operation_id = "close_risk",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body(content = RiskPatch, content_type = "application/merge-patch+json"),
    responses(
        (status = 200, body = RiskBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid patch", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn close<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(patch): JsonBody<RiskPatch>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = RiskId::from_uuid(id);
    set_closed(&state.store, acting, id, expected, patch.closed).await?;
    Ok(with_etag(
        StatusCode::OK,
        get_risk(&state.store, id).await?.into(),
    ))
}
