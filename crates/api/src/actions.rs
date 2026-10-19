//! `/api/v1/actions`: list, create, fetch, edit, close and reopen Actions
//! (FR-30 to FR-33).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveDate;
use lodestar_core::{
    action::{
        ActionId, ActionView,
        use_cases::{ActionEdit, NewAction},
    },
    person::PersonId,
    ports::{ActionFilter, PageRequest},
    project::ProjectId,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState,
    blockers::ItemRefBody,
    people::{DEFAULT_LIMIT, MAX_LIMIT},
    problem::Problem,
    projects::{PersonRefBody, not_allowed, present},
    request::{JsonBody, Path, Query, etag, if_match},
    session::Acting,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/actions",
            get(list).post(create).delete(not_allowed("GET,HEAD,POST")),
        )
        .route(
            "/actions/{id}",
            get(fetch)
                .put(update)
                .patch(close)
                .delete(not_allowed("GET,HEAD,PUT,PATCH")),
        )
}

/// An Action. Also the request body for `POST` and `PUT`: read-only fields
/// may be sent to `PUT` only if they match what is stored.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = Action)]
pub struct ActionBody {
    #[schema(read_only)]
    pub id: Uuid,
    /// Required on `POST`; never changes.
    pub project_id: Uuid,
    #[schema(read_only)]
    pub project_code: String,
    /// `A-001`, … numbered within the Project. A label, not an identifier.
    #[schema(read_only)]
    pub reference: String,
    /// 1–4000 characters once trimmed; may span lines.
    #[schema(min_length = 1, max_length = 4000)]
    pub description: String,
    /// A Member, or `null` when unassigned. Only `id` is read. Required on `PUT`.
    pub assignee: Option<PersonRefBody>,
    /// `YYYY-MM-DD`; past dates are allowed. Required on `PUT`.
    #[schema(value_type = Option<String>, format = Date)]
    pub due_date: Option<NaiveDate>,
    /// Changed only with `PATCH`.
    #[schema(read_only)]
    pub closed: bool,
    /// The open Blockers blocking this Action, ordered by Reference.
    #[schema(read_only)]
    pub blocked_by: Vec<ItemRefBody>,
    #[schema(read_only, minimum = 1)]
    pub version: u32,
}

impl From<ActionView> for ActionBody {
    fn from(view: ActionView) -> Self {
        Self {
            id: view.id.as_uuid(),
            project_id: view.project_id.as_uuid(),
            project_code: view.project_code,
            reference: view.reference,
            description: view.description,
            assignee: view.assignee.map(|a| PersonRefBody {
                id: a.id.as_uuid(),
                display_name: a.display_name,
            }),
            due_date: view.due_date,
            closed: view.closed,
            blocked_by: view.blocked_by.into_iter().map(ItemRefBody::from).collect(),
            version: view.version,
        }
    }
}

/// What a client may send for an Action; documented as [`ActionBody`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionInput {
    id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_code: Option<String>,
    reference: Option<String>,
    description: String,
    #[serde(default, deserialize_with = "present")]
    assignee: Option<Option<AssigneeInput>>,
    #[serde(default, deserialize_with = "present")]
    due_date: Option<Option<NaiveDate>>,
    closed: Option<bool>,
    blocked_by: Option<serde_json::Value>,
    version: Option<u32>,
}

/// The assignee as sent: only `id` matters; `display_name` may be echoed back.
#[derive(Debug, Deserialize)]
struct AssigneeInput {
    id: Uuid,
    #[allow(dead_code)]
    display_name: Option<String>,
}

fn assignee(input: Option<&AssigneeInput>) -> Option<PersonId> {
    input.map(|a| PersonId::from_uuid(a.id))
}

/// The only change `PATCH` makes (ADR-0006).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionPatch {
    pub closed: bool,
}

fn with_etag(status: StatusCode, action: ActionBody) -> Response {
    (status, [(header::ETAG, etag(action.version))], Json(action)).into_response()
}

/// A page of Actions.
#[derive(Debug, Serialize, ToSchema)]
pub struct ActionPage {
    pub items: Vec<ActionBody>,
    /// Matching Actions across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Only this Project's Actions.
    project_id: Option<Uuid>,
    /// Page size, 1–1000.
    #[param(minimum = 1, maximum = 1000, default = 50)]
    limit: Option<u32>,
    #[param(minimum = 0, default = 0)]
    offset: Option<u32>,
    /// Include closed Actions.
    #[param(default = false)]
    include_closed: Option<bool>,
}

/// Lists Actions by Project Code, then Reference. Closed Actions are left out
/// unless `include_closed=true`.
#[utoipa::path(
    get,
    path = "/api/v1/actions",
    tag = "actions",
    operation_id = "list_actions",
    params(ListParams),
    responses(
        (status = 200, body = ActionPage),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<ActionPage>, Problem> {
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(Problem::invalid(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let offset = params.offset.unwrap_or(0);
    let filter = ActionFilter {
        project_id: params.project_id.map(ProjectId::from_uuid),
        include_closed: params.include_closed.unwrap_or(false),
    };
    let page = state
        .actions
        .list(filter, PageRequest { offset, limit })
        .await?;
    Ok(Json(ActionPage {
        items: page.items.into_iter().map(ActionBody::from).collect(),
        total: page.total,
        limit,
        offset,
    }))
}

/// Creates an open Action with the next Reference in its Project.
#[utoipa::path(
    post,
    path = "/api/v1/actions",
    tag = "actions",
    operation_id = "create_action",
    request_body = ActionBody,
    responses(
        (status = 201, body = ActionBody, headers(("location" = String), ("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Action", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create(
    State(state): State<AppState>,
    Acting(acting): Acting,
    JsonBody(input): JsonBody<ActionInput>,
) -> Result<Response, Problem> {
    if input.id.is_some()
        || input.project_code.is_some()
        || input.reference.is_some()
        || input.closed.is_some()
        || input.blocked_by.is_some()
        || input.version.is_some()
    {
        return Err(Problem::invalid(
            "id, project_code, reference, closed, blocked_by and version are set by the server",
        ));
    }
    let Some(project_id) = input.project_id else {
        return Err(Problem::invalid("project_id is required"));
    };
    let action = state
        .actions
        .create(
            acting,
            NewAction {
                project_id: ProjectId::from_uuid(project_id),
                description: &input.description,
                assignee: assignee(input.assignee.flatten().as_ref()),
                due_date: input.due_date.flatten(),
            },
        )
        .await?;
    let body = ActionBody::from(state.actions.get(action.id()).await?);
    let location = format!("/api/v1/actions/{}", body.id);
    Ok((
        [(header::LOCATION, location)],
        with_etag(StatusCode::CREATED, body),
    )
        .into_response())
}

/// Fetches an Action, open or closed.
#[utoipa::path(
    get,
    path = "/api/v1/actions/{id}",
    tag = "actions",
    operation_id = "get_action",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = ActionBody, headers(("etag" = String))),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
async fn fetch(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let view = state.actions.get(ActionId::from_uuid(id)).await?;
    Ok(with_etag(StatusCode::OK, view.into()))
}

/// Replaces an Action's description, assignee and due date. Needs a session
/// and `If-Match`.
#[utoipa::path(
    put,
    path = "/api/v1/actions/{id}",
    tag = "actions",
    operation_id = "update_action",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body = ActionBody,
    responses(
        (status = 200, body = ActionBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Action", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn update(
    State(state): State<AppState>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<ActionInput>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if input.version.is_some_and(|v| v != expected) {
        return Err(Problem::stale());
    }
    if input.id.is_some_and(|body_id| body_id != id) {
        return Err(Problem::invalid("id does not match the URL"));
    }
    let Some(assignee_input) = input.assignee else {
        return Err(Problem::invalid("assignee is required (null unassigns)"));
    };
    let Some(due_date) = input.due_date else {
        return Err(Problem::invalid("due_date is required (null clears it)"));
    };
    let id = ActionId::from_uuid(id);
    let stored = state.actions.get(id).await?;
    if stored.version != expected {
        return Err(Problem::stale());
    }
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
    state
        .actions
        .update(
            acting,
            id,
            expected,
            ActionEdit {
                description: &input.description,
                assignee: assignee(assignee_input.as_ref()),
                due_date,
            },
        )
        .await?;
    Ok(with_etag(
        StatusCode::OK,
        state.actions.get(id).await?.into(),
    ))
}

/// Closes or reopens an Action, with `{"closed": …}` as
/// `application/merge-patch+json`. Needs a session and `If-Match`.
#[utoipa::path(
    patch,
    path = "/api/v1/actions/{id}",
    tag = "actions",
    operation_id = "close_action",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body(content = ActionPatch, content_type = "application/merge-patch+json"),
    responses(
        (status = 200, body = ActionBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid patch", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn close(
    State(state): State<AppState>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(patch): JsonBody<ActionPatch>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = ActionId::from_uuid(id);
    state
        .actions
        .set_closed(acting, id, expected, patch.closed)
        .await?;
    Ok(with_etag(
        StatusCode::OK,
        state.actions.get(id).await?.into(),
    ))
}
