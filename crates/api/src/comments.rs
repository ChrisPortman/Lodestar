//! `/api/v1/comments`: list, add, fetch, edit and delete Comments on a
//! Project or its items (FR-120 to FR-126).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use lodestar_core::{
    comment::{
        CommentId, CommentView, Target, TargetType,
        use_cases::{
            CommentFilter, NewComment, create_comment, delete_comment, edit_comment, get_comment,
            list_comments,
        },
    },
    project::ProjectId,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState, Store,
    problem::Problem,
    refs::PersonRefBody,
    request::{JsonBody, Path, Query, etag, if_match, not_allowed, page_request, present},
    session::Writer,
};

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new()
        .route(
            "/comments",
            get(list::<S>)
                .post(create::<S>)
                .delete(not_allowed("GET,HEAD,POST")),
        )
        .route(
            "/comments/{id}",
            get(fetch::<S>)
                .put(update::<S>)
                .patch(delete::<S>)
                .delete(not_allowed("GET,HEAD,PUT,PATCH")),
        )
}

/// What a Comment is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetTypeParam {
    Project,
    Action,
    Risk,
    Blocker,
    Meeting,
}

impl From<TargetTypeParam> for TargetType {
    fn from(kind: TargetTypeParam) -> Self {
        match kind {
            TargetTypeParam::Project => Self::Project,
            TargetTypeParam::Action => Self::Action,
            TargetTypeParam::Risk => Self::Risk,
            TargetTypeParam::Blocker => Self::Blocker,
            TargetTypeParam::Meeting => Self::Meeting,
        }
    }
}

impl From<TargetType> for TargetTypeParam {
    fn from(kind: TargetType) -> Self {
        match kind {
            TargetType::Project => Self::Project,
            TargetType::Action => Self::Action,
            TargetType::Risk => Self::Risk,
            TargetType::Blocker => Self::Blocker,
            TargetType::Meeting => Self::Meeting,
        }
    }
}

/// A Comment. Also the request body for `POST` and `PUT`: read-only fields
/// may be sent to `PUT` only if they match what is stored.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = Comment)]
pub struct CommentBody {
    #[schema(read_only)]
    pub id: Uuid,
    /// Required on `POST`; never changes.
    pub project_id: Uuid,
    /// Required on `POST`; never changes.
    pub target_type: TargetTypeParam,
    /// The Project's ID when `target_type` is `project`. Required on `POST`;
    /// never changes.
    pub target_id: Uuid,
    /// The Acting Person who added it.
    #[schema(read_only)]
    pub author: PersonRefBody,
    /// Plain text, kept exactly as written: 1–4000 characters, not blank.
    /// Empty once the Comment is deleted.
    #[schema(max_length = 4000)]
    pub body: String,
    #[schema(read_only, value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    /// When the text was last edited, if ever.
    #[schema(read_only, value_type = Option<String>, format = DateTime)]
    pub edited_at: Option<DateTime<Utc>>,
    /// Changed only with `PATCH`, and only to `true`.
    #[schema(read_only)]
    pub deleted: bool,
    #[schema(read_only, minimum = 1)]
    pub version: u32,
}

impl CommentBody {
    /// The Comment with its text, even when deleted: for the Project Export.
    pub fn revealed(view: CommentView) -> Self {
        Self {
            id: view.id.as_uuid(),
            project_id: view.project_id.as_uuid(),
            target_type: view.target.kind.into(),
            target_id: view.target.id,
            author: view.author.into(),
            body: view.body,
            created_at: view.created_at,
            edited_at: view.edited_at,
            deleted: view.deleted,
            version: view.version,
        }
    }
}

/// The API never returns a deleted Comment's text (FR-123).
impl From<CommentView> for CommentBody {
    fn from(view: CommentView) -> Self {
        let mut body = Self::revealed(view);
        if body.deleted {
            body.body.clear();
        }
        body
    }
}

/// What a client may send for a Comment; documented as [`CommentBody`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommentInput {
    id: Option<Uuid>,
    project_id: Option<Uuid>,
    target_type: Option<TargetTypeParam>,
    target_id: Option<Uuid>,
    author: Option<PersonRefBody>,
    body: String,
    created_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "present")]
    edited_at: Option<Option<DateTime<Utc>>>,
    deleted: Option<bool>,
    version: Option<u32>,
}

/// The only change `PATCH` makes (ADR-0006): deleting, which can't be undone.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentPatch {
    /// Must be `true`.
    pub deleted: bool,
}

fn with_etag(status: StatusCode, comment: CommentBody) -> Response {
    (
        status,
        [(header::ETAG, etag(comment.version))],
        Json(comment),
    )
        .into_response()
}

/// A page of Comments.
#[derive(Debug, Serialize, ToSchema)]
pub struct CommentPage {
    pub items: Vec<CommentBody>,
    /// Matching Comments across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// The Project whose Comments to list, including those on its items.
    project_id: Uuid,
    /// Only Comments about this kind of target; needs `target_id`.
    #[param(inline)]
    target_type: Option<TargetTypeParam>,
    /// Only Comments about this target; needs `target_type`.
    target_id: Option<Uuid>,
    /// Page size, 1–1000.
    #[param(minimum = 1, maximum = 1000, default = 50)]
    limit: Option<u32>,
    #[param(minimum = 0, default = 0)]
    offset: Option<u32>,
}

/// Lists a Project's Comments oldest first, deleted ones included.
#[utoipa::path(
    get,
    path = "/api/v1/comments",
    tag = "comments",
    operation_id = "list_comments",
    params(ListParams),
    responses(
        (status = 200, body = CommentPage),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn list<S: Store>(
    State(state): State<AppState<S>>,
    Query(params): Query<ListParams>,
) -> Result<Json<CommentPage>, Problem> {
    let page = page_request(params.limit, params.offset)?;
    let target = match (params.target_type, params.target_id) {
        (None, None) => None,
        (Some(kind), Some(id)) => Some(Target {
            kind: kind.into(),
            id,
        }),
        _ => {
            return Err(Problem::invalid(
                "target_type and target_id must be given together",
            ));
        }
    };
    let filter = CommentFilter {
        project_id: Some(ProjectId::from_uuid(params.project_id)),
        target,
    };
    let found = list_comments(&state.store, filter, page).await?;
    Ok(Json(CommentPage {
        items: found.items.into_iter().map(CommentBody::from).collect(),
        total: found.total,
        limit: page.limit,
        offset: page.offset,
    }))
}

/// Adds a Comment by the Acting Person, who must be a Member.
#[utoipa::path(
    post,
    path = "/api/v1/comments",
    tag = "comments",
    operation_id = "create_comment",
    request_body = CommentBody,
    responses(
        (status = 201, body = CommentBody, headers(("location" = String), ("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a Member", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Comment", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    JsonBody(input): JsonBody<CommentInput>,
) -> Result<Response, Problem> {
    if input.id.is_some()
        || input.author.is_some()
        || input.created_at.is_some()
        || input.edited_at.is_some()
        || input.deleted.is_some()
        || input.version.is_some()
    {
        return Err(Problem::invalid(
            "id, author, created_at, edited_at, deleted and version are set by the server",
        ));
    }
    let (Some(project_id), Some(kind), Some(target_id)) =
        (input.project_id, input.target_type, input.target_id)
    else {
        return Err(Problem::invalid(
            "project_id, target_type and target_id are required",
        ));
    };
    let comment = create_comment(
        &state.store,
        acting,
        NewComment {
            project_id: ProjectId::from_uuid(project_id),
            target: Target {
                kind: kind.into(),
                id: target_id,
            },
            body: &input.body,
        },
    )
    .await?;
    let body = CommentBody::from(get_comment(&state.store, comment.id()).await?);
    let location = format!("/api/v1/comments/{}", body.id);
    Ok((
        [(header::LOCATION, location)],
        with_etag(StatusCode::CREATED, body),
    )
        .into_response())
}

/// Fetches a Comment. A deleted Comment's `body` is empty.
#[utoipa::path(
    get,
    path = "/api/v1/comments/{id}",
    tag = "comments",
    operation_id = "get_comment",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = CommentBody, headers(("etag" = String))),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
async fn fetch<S: Store>(
    State(state): State<AppState<S>>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    let view = get_comment(&state.store, CommentId::from_uuid(id)).await?;
    Ok(with_etag(StatusCode::OK, view.into()))
}

/// Replaces a Comment's text. Only its author can, while a Member. Needs a
/// session and `If-Match`.
#[utoipa::path(
    put,
    path = "/api/v1/comments/{id}",
    tag = "comments",
    operation_id = "update_comment",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body = CommentBody,
    responses(
        (status = 200, body = CommentBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not the author, or no longer a Member", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, another session holds its live Meeting Lock, or the Comment is deleted", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Comment", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn update<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<CommentInput>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if input.version.is_some_and(|v| v != expected) {
        return Err(Problem::stale());
    }
    if input.id.is_some_and(|body_id| body_id != id) {
        return Err(Problem::invalid("id does not match the URL"));
    }
    let id = CommentId::from_uuid(id);
    let stored = get_comment(&state.store, id).await?;
    let mismatch = input
        .project_id
        .is_some_and(|p| p != stored.project_id.as_uuid())
        || input
            .target_type
            .is_some_and(|t| TargetType::from(t) != stored.target.kind)
        || input.target_id.is_some_and(|t| t != stored.target.id)
        || input
            .author
            .as_ref()
            .is_some_and(|a| a.id != stored.author.id.as_uuid())
        || input.created_at.is_some_and(|c| c != stored.created_at)
        || input.edited_at.is_some_and(|e| e != stored.edited_at)
        || input.deleted.is_some_and(|d| d != stored.deleted);
    if mismatch {
        return Err(Problem::invalid(
            "project_id, target_type, target_id, author, created_at, edited_at and deleted \
             cannot be changed with PUT",
        ));
    }
    edit_comment(&state.store, acting, id, expected, &input.body).await?;
    Ok(with_etag(
        StatusCode::OK,
        get_comment(&state.store, id).await?.into(),
    ))
}

/// Deletes a Comment, with `{"deleted": true}` as
/// `application/merge-patch+json`. It can't be undone. Only its author can,
/// while a Member. Needs a session and `If-Match`.
#[utoipa::path(
    patch,
    path = "/api/v1/comments/{id}",
    tag = "comments",
    operation_id = "delete_comment",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body(content = CommentPatch, content_type = "application/merge-patch+json"),
    responses(
        (status = 200, body = CommentBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not the author, or no longer a Member", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived, or another session holds its live Meeting Lock", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid patch", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn delete<S: Store>(
    State(state): State<AppState<S>>,
    Writer(acting): Writer,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(patch): JsonBody<CommentPatch>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if !patch.deleted {
        return Err(Problem::invalid("a deleted comment cannot be restored"));
    }
    let id = CommentId::from_uuid(id);
    delete_comment(&state.store, acting, id, expected).await?;
    Ok(with_etag(
        StatusCode::OK,
        get_comment(&state.store, id).await?.into(),
    ))
}
