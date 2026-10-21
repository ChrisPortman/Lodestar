//! `/api/v1/blockers`: list, create, fetch, edit, clear and reopen Blockers,
//! and link them to the Actions they block (FR-50 to FR-54).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use lodestar_core::{
    action::ActionId,
    blocker::{
        BlockerId, BlockerView,
        use_cases::{
            BlockerEdit, NewBlocker, create_blocker, get_blocker, link_blocked_action,
            list_blockers, set_blocker_cleared, unlink_blocked_action, update_blocker,
        },
    },
    person::PersonId,
    ports::{BlockerFilter, BlockerSort, PageRequest},
    project::ProjectId,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState, Store,
    filters::{self, BlockerStateParam},
    problem::Problem,
    refs::{ItemRefBody, PersonRefBody},
    request::{JsonBody, Path, Query, etag, if_match, not_allowed, page_request, present},
    session::Writer,
};

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new()
        .route(
            "/blockers",
            get(list::<S>)
                .post(create::<S>)
                .delete(not_allowed("GET,HEAD,POST")),
        )
        .route(
            "/blockers/{id}",
            get(fetch::<S>)
                .put(update::<S>)
                .patch(clear::<S>)
                .delete(not_allowed("GET,HEAD,PUT,PATCH")),
        )
        .route("/blockers/{id}/blocked_actions", post(link::<S>))
        .route(
            "/blockers/{id}/blocked_actions/{action_id}",
            delete(unlink::<S>),
        )
}

/// A Blocker. Also the request body for `POST` and `PUT`: read-only fields
/// may be sent to `PUT` only if they match what is stored.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = Blocker)]
pub struct BlockerBody {
    #[schema(read_only)]
    pub id: Uuid,
    /// Required on `POST`; never changes.
    pub project_id: Uuid,
    #[schema(read_only)]
    pub project_code: String,
    /// `B-001`, … numbered within the Project. A label, not an identifier.
    #[schema(read_only)]
    pub reference: String,
    /// 1–4000 characters once trimmed; may span lines.
    #[schema(min_length = 1, max_length = 4000)]
    pub description: String,
    /// The accountable Member, or `null`. Only `id` is read. Required on `PUT`.
    pub accountable: Option<PersonRefBody>,
    /// Changed only with `PATCH`.
    #[schema(read_only)]
    pub cleared: bool,
    /// The Actions this Blocker blocks, ordered by Reference. Changed only
    /// through `/blockers/{id}/blocked_actions`.
    #[schema(read_only)]
    pub blocked_actions: Vec<ItemRefBody>,
    #[schema(read_only, minimum = 1)]
    pub version: u32,
}

impl From<BlockerView> for BlockerBody {
    fn from(view: BlockerView) -> Self {
        Self {
            id: view.id.as_uuid(),
            project_id: view.project_id.as_uuid(),
            project_code: view.project_code,
            reference: view.reference,
            description: view.description,
            accountable: view.accountable.map(PersonRefBody::from),
            cleared: view.cleared,
            blocked_actions: view
                .blocked_actions
                .into_iter()
                .map(ItemRefBody::from)
                .collect(),
            version: view.version,
        }
    }
}

/// What a client may send for a Blocker; documented as [`BlockerBody`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockerInput {
    id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_code: Option<String>,
    reference: Option<String>,
    description: String,
    #[serde(default, deserialize_with = "present")]
    accountable: Option<Option<AccountableInput>>,
    cleared: Option<bool>,
    blocked_actions: Option<Vec<ItemRefBody>>,
    version: Option<u32>,
}

/// The accountable Member as sent: only `id` matters.
#[derive(Debug, Deserialize)]
struct AccountableInput {
    id: Uuid,
    #[allow(dead_code)]
    display_name: Option<String>,
}

fn accountable(input: Option<&AccountableInput>) -> Option<PersonId> {
    input.map(|a| PersonId::from_uuid(a.id))
}

/// The only change `PATCH` makes (ADR-0006).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockerPatch {
    pub cleared: bool,
}

/// The Action a Blocker blocks.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewBlockedAction {
    pub action_id: Uuid,
}

fn with_etag(status: StatusCode, blocker: BlockerBody) -> Response {
    (
        status,
        [(header::ETAG, etag(blocker.version))],
        Json(blocker),
    )
        .into_response()
}

/// A page of Blockers.
#[derive(Debug, Serialize, ToSchema)]
pub struct BlockerPage {
    pub items: Vec<BlockerBody>,
    /// Matching Blockers across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// The order to list Blockers in.
#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockerSortParam {
    /// Creation order.
    #[default]
    Id,
    /// Project Code, then Reference number.
    Reference,
    /// Accountable display name; unassigned last.
    Accountable,
}

impl From<BlockerSortParam> for BlockerSort {
    fn from(sort: BlockerSortParam) -> Self {
        match sort {
            BlockerSortParam::Id => Self::Id,
            BlockerSortParam::Reference => Self::Reference,
            BlockerSortParam::Accountable => BlockerSort::Accountable,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Only this Project's Blockers.
    project_id: Option<Uuid>,
    /// Page size, 1–1000.
    #[param(minimum = 1, maximum = 1000, default = 50)]
    limit: Option<u32>,
    #[param(minimum = 0, default = 0)]
    offset: Option<u32>,
    #[param(inline)]
    sort: Option<BlockerSortParam>,
    /// Open (the default), cleared or all Blockers.
    #[param(inline)]
    state: Option<BlockerStateParam>,
    /// Only Blockers this Person ID is accountable for, or `unassigned`.
    accountable: Option<String>,
}

/// Lists Blockers, ordered by ID unless `sort` is given. Only open Blockers are
/// listed unless `state` says otherwise.
#[utoipa::path(
    get,
    path = "/api/v1/blockers",
    tag = "blockers",
    operation_id = "list_blockers",
    params(ListParams),
    responses(
        (status = 200, body = BlockerPage),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn list<S: Store>(
    State(state): State<AppState<S>>,
    Query(params): Query<ListParams>,
) -> Result<Json<BlockerPage>, Problem> {
    let PageRequest { offset, limit } = page_request(params.limit, params.offset)?;
    let filter = BlockerFilter {
        project_id: params.project_id.map(ProjectId::from_uuid),
        state: params.state.unwrap_or_default().into(),
        accountable: filters::assignee(params.accountable.as_deref(), "accountable")?,
    };
    let page = list_blockers(
        &state.store,
        filter,
        PageRequest { offset, limit },
        params.sort.unwrap_or_default().into(),
    )
    .await?;
    Ok(Json(BlockerPage {
        items: page.items.into_iter().map(BlockerBody::from).collect(),
        total: page.total,
        limit,
        offset,
    }))
}

/// Creates an open Blocker with the next Reference in its Project.
#[utoipa::path(
    post,
    path = "/api/v1/blockers",
    tag = "blockers",
    operation_id = "create_blocker",
    request_body = BlockerBody,
    responses(
        (status = 201, body = BlockerBody, headers(("location" = String), ("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Blocker", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    JsonBody(input): JsonBody<BlockerInput>,
) -> Result<Response, Problem> {
    if input.id.is_some()
        || input.project_code.is_some()
        || input.reference.is_some()
        || input.cleared.is_some()
        || input.blocked_actions.is_some()
        || input.version.is_some()
    {
        return Err(Problem::invalid(
            "id, project_code, reference, cleared, blocked_actions and version are set by the server",
        ));
    }
    let Some(project_id) = input.project_id else {
        return Err(Problem::invalid("project_id is required"));
    };
    let blocker = create_blocker(
        &state.store,
        acting,
        NewBlocker {
            project_id: ProjectId::from_uuid(project_id),
            description: &input.description,
            accountable: accountable(input.accountable.flatten().as_ref()),
        },
    )
    .await?;
    let body = BlockerBody::from(get_blocker(&state.store, blocker.id()).await?);
    let location = format!("/api/v1/blockers/{}", body.id);
    Ok((
        [(header::LOCATION, location)],
        with_etag(StatusCode::CREATED, body),
    )
        .into_response())
}

/// Fetches a Blocker, open or cleared.
#[utoipa::path(
    get,
    path = "/api/v1/blockers/{id}",
    tag = "blockers",
    operation_id = "get_blocker",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = BlockerBody, headers(("etag" = String))),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
async fn fetch<S: Store>(
    State(state): State<AppState<S>>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    let view = get_blocker(&state.store, BlockerId::from_uuid(id)).await?;
    Ok(with_etag(StatusCode::OK, view.into()))
}

/// Replaces a Blocker's description and accountable Member. Needs a session
/// and `If-Match`.
#[utoipa::path(
    put,
    path = "/api/v1/blockers/{id}",
    tag = "blockers",
    operation_id = "update_blocker",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body = BlockerBody,
    responses(
        (status = 200, body = BlockerBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Blocker", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn update<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<BlockerInput>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if input.version.is_some_and(|v| v != expected) {
        return Err(Problem::stale());
    }
    if input.id.is_some_and(|body_id| body_id != id) {
        return Err(Problem::invalid("id does not match the URL"));
    }
    let Some(accountable_input) = input.accountable else {
        return Err(Problem::invalid("accountable is required (null unassigns)"));
    };
    let id = BlockerId::from_uuid(id);
    let stored = get_blocker(&state.store, id).await?;
    if stored.version != expected {
        return Err(Problem::stale());
    }
    let stored_links: Vec<Uuid> = stored.blocked_actions.iter().map(|a| a.id).collect();
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
        || input.cleared.is_some_and(|c| c != stored.cleared)
        || input
            .blocked_actions
            .as_ref()
            .is_some_and(|links| links.iter().map(|a| a.id).ne(stored_links.iter().copied()));
    if mismatch {
        return Err(Problem::invalid(
            "project_id, project_code, reference, cleared and blocked_actions cannot be changed with PUT",
        ));
    }
    update_blocker(
        &state.store,
        acting,
        id,
        expected,
        BlockerEdit {
            description: &input.description,
            accountable: accountable(accountable_input.as_ref()),
        },
    )
    .await?;
    Ok(with_etag(
        StatusCode::OK,
        get_blocker(&state.store, id).await?.into(),
    ))
}

/// Clears or reopens a Blocker, with `{"cleared": …}` as
/// `application/merge-patch+json`. Needs a session and `If-Match`.
#[utoipa::path(
    patch,
    path = "/api/v1/blockers/{id}",
    tag = "blockers",
    operation_id = "clear_blocker",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body(content = BlockerPatch, content_type = "application/merge-patch+json"),
    responses(
        (status = 200, body = BlockerBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid patch", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn clear<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(patch): JsonBody<BlockerPatch>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = BlockerId::from_uuid(id);
    set_blocker_cleared(&state.store, acting, id, expected, patch.cleared).await?;
    Ok(with_etag(
        StatusCode::OK,
        get_blocker(&state.store, id).await?.into(),
    ))
}

/// Links the Blocker to an Action in the same Project that it blocks, in
/// any state. Linking an already linked Action changes nothing. Needs a
/// session and the Blocker's `If-Match`.
#[utoipa::path(
    post,
    path = "/api/v1/blockers/{id}/blocked_actions",
    tag = "blockers",
    operation_id = "link_blocked_action",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the Blocker version being changed")),
    request_body = NewBlockedAction,
    responses(
        (status = 200, body = BlockerBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "No such Action, or in another Project", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn link<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<NewBlockedAction>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = BlockerId::from_uuid(id);
    link_blocked_action(
        &state.store,
        acting,
        id,
        expected,
        ActionId::from_uuid(input.action_id),
    )
    .await?;
    Ok(with_etag(
        StatusCode::OK,
        get_blocker(&state.store, id).await?.into(),
    ))
}

/// Removes the Blocker's link to an Action, in any state. Unlinking an
/// Action that is not linked changes nothing. Needs a session and the
/// Blocker's `If-Match`.
#[utoipa::path(
    delete,
    path = "/api/v1/blockers/{id}/blocked_actions/{action_id}",
    tag = "blockers",
    operation_id = "unlink_blocked_action",
    params(("id" = Uuid, Path), ("action_id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the Blocker version being changed")),
    responses(
        (status = 200, body = BlockerBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn unlink<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    Path((id, action_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = BlockerId::from_uuid(id);
    unlink_blocked_action(
        &state.store,
        acting,
        id,
        expected,
        ActionId::from_uuid(action_id),
    )
    .await?;
    Ok(with_etag(
        StatusCode::OK,
        get_blocker(&state.store, id).await?.into(),
    ))
}
