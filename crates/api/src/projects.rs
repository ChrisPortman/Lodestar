//! `/api/v1/projects`: list, create, fetch, edit and archive Projects
//! (FR-10 to FR-18), and Project Code suggestions (FR-16).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use lodestar_core::{
    person::PersonId,
    ports::{PageRequest, ProjectSort},
    project::{
        HealthRating, ProjectId, ProjectView, Rag,
        use_cases::{
            HealthRatingInput, NewProject, ProjectEdit, create_project, get_project, list_projects,
            set_archived, suggest_project_code, update_project,
        },
    },
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState, Store,
    problem::Problem,
    refs::PersonRefBody,
    request::{JsonBody, Path, Query, etag, if_match, not_allowed, page_request, present},
    session::Acting,
};

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new()
        .route(
            "/projects",
            get(list::<S>)
                .post(create::<S>)
                .delete(not_allowed("GET,HEAD,POST")),
        )
        .route("/projects/code_suggestion", get(suggest_code::<S>))
        .route(
            "/projects/{id}",
            get(fetch::<S>)
                .put(update::<S>)
                .patch(archive::<S>)
                .delete(not_allowed("GET,HEAD,PUT,PATCH")),
        )
}

/// Red, Amber or Green.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = Rag)]
pub enum RagBody {
    Red,
    Amber,
    Green,
}

impl From<Rag> for RagBody {
    fn from(rag: Rag) -> Self {
        match rag {
            Rag::Red => Self::Red,
            Rag::Amber => Self::Amber,
            Rag::Green => Self::Green,
        }
    }
}

impl From<RagBody> for Rag {
    fn from(rag: RagBody) -> Self {
        match rag {
            RagBody::Red => Self::Red,
            RagBody::Amber => Self::Amber,
            RagBody::Green => Self::Green,
        }
    }
}

/// A Project's Health Rating, set by hand (FR-70).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = HealthRating)]
pub struct HealthRatingBody {
    pub rating: RagBody,
    /// 1–4000 characters once trimmed, or `null`.
    #[schema(min_length = 1, max_length = 4000)]
    #[serde(default)]
    pub comment: Option<String>,
}

impl From<HealthRating> for HealthRatingBody {
    fn from(h: HealthRating) -> Self {
        Self {
            rating: h.rating.into(),
            comment: h.comment.map(String::from),
        }
    }
}

/// A Project. Also the request body for `POST` and `PUT`: read-only fields may
/// be sent to `PUT` only if they match what is stored.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = Project)]
pub struct ProjectBody {
    #[schema(read_only)]
    pub id: Uuid,
    /// Required, 1–100 characters; surrounding whitespace is trimmed.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// 1–4000 characters once trimmed, or `null`. Required on `PUT`.
    #[schema(min_length = 1, max_length = 4000)]
    pub description: Option<String>,
    /// Set on creation and never changed: an uppercase letter followed by
    /// 1–9 uppercase letters or digits, unique across Projects.
    #[schema(pattern = "^[A-Z][A-Z0-9]{1,9}$")]
    pub project_code: String,
    /// Changed only with `PATCH`.
    #[schema(read_only)]
    pub archived: bool,
    /// The creator, to begin with. Must be a Member; `null` means no
    /// Project Leader. Required on `PUT`, where only `id` is read.
    pub project_leader: Option<PersonRefBody>,
    /// `null` when the Project is not rated. Required on `PUT`.
    pub health_rating: Option<HealthRatingBody>,
    #[schema(read_only, minimum = 1)]
    pub version: u32,
}

impl From<ProjectView> for ProjectBody {
    fn from(view: ProjectView) -> Self {
        Self {
            id: view.id.as_uuid(),
            name: view.name,
            description: view.description,
            project_code: view.project_code,
            archived: view.archived,
            project_leader: view.project_leader.map(PersonRefBody::from),
            health_rating: view.health_rating.map(HealthRatingBody::from),
            version: view.version,
        }
    }
}

/// What a client may send for a Project; documented as [`ProjectBody`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectInput {
    id: Option<Uuid>,
    name: String,
    #[serde(default, deserialize_with = "present")]
    description: Option<Option<String>>,
    project_code: Option<String>,
    archived: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    project_leader: Option<Option<LeaderInput>>,
    #[serde(default, deserialize_with = "present")]
    health_rating: Option<Option<HealthRatingBody>>,
    version: Option<u32>,
}

/// The Project Leader as sent: only `id` matters; `display_name` may be echoed back.
#[derive(Debug, Deserialize)]
struct LeaderInput {
    id: Uuid,
    #[allow(dead_code)]
    display_name: Option<String>,
}

/// The only change `PATCH` makes (ADR-0006).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectPatch {
    pub archived: bool,
}

pub(crate) fn with_etag(status: StatusCode, project: ProjectBody) -> Response {
    (
        status,
        [(header::ETAG, etag(project.version))],
        Json(project),
    )
        .into_response()
}

/// A page of Projects.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectPage {
    pub items: Vec<ProjectBody>,
    /// Matching Projects across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSortParam {
    /// Creation order.
    #[default]
    Id,
    Name,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Page size, 1–1000.
    #[param(minimum = 1, maximum = 1000, default = 50)]
    limit: Option<u32>,
    #[param(minimum = 0, default = 0)]
    offset: Option<u32>,
    #[param(inline)]
    sort: Option<ProjectSortParam>,
    /// Include archived Projects.
    #[param(default = false)]
    include_archived: Option<bool>,
}

/// Lists Projects, ordered by ID unless `sort` is given. Archived Projects
/// are left out unless `include_archived=true`.
#[utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "projects",
    operation_id = "list_projects",
    params(ListParams),
    responses(
        (status = 200, body = ProjectPage),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn list<S: Store>(
    State(state): State<AppState<S>>,
    Query(params): Query<ListParams>,
) -> Result<Json<ProjectPage>, Problem> {
    let PageRequest { offset, limit } = page_request(params.limit, params.offset)?;
    let sort = match params.sort.unwrap_or_default() {
        ProjectSortParam::Id => ProjectSort::Id,
        ProjectSortParam::Name => ProjectSort::Name,
    };
    let page = list_projects(
        &state.store,
        PageRequest { offset, limit },
        params.include_archived.unwrap_or(false),
        sort,
    )
    .await?;
    Ok(Json(ProjectPage {
        items: page.items.into_iter().map(ProjectBody::from).collect(),
        total: page.total,
        limit,
        offset,
    }))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SuggestionParams {
    /// The Project name to suggest a code for.
    name: String,
}

/// A suggested Project Code.
#[derive(Debug, Serialize, ToSchema)]
pub struct CodeSuggestion {
    /// `null` when the name has too few letters to suggest one.
    pub project_code: Option<String>,
}

/// Suggests an unused Project Code for a name (FR-16).
#[utoipa::path(
    get,
    path = "/api/v1/projects/code_suggestion",
    tag = "projects",
    operation_id = "suggest_project_code",
    params(SuggestionParams),
    responses(
        (status = 200, body = CodeSuggestion),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn suggest_code<S: Store>(
    State(state): State<AppState<S>>,
    Query(params): Query<SuggestionParams>,
) -> Result<Json<CodeSuggestion>, Problem> {
    let code = suggest_project_code(&state.store, &params.name).await?;
    Ok(Json(CodeSuggestion {
        project_code: code.map(String::from),
    }))
}

/// Creates a Project. The Acting Person becomes a Member and its Project Leader.
#[utoipa::path(
    post,
    path = "/api/v1/projects",
    tag = "projects",
    operation_id = "create_project",
    request_body = ProjectBody,
    responses(
        (status = 201, body = ProjectBody, headers(("location" = String), ("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Project Code in use", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Project", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    JsonBody(input): JsonBody<ProjectInput>,
) -> Result<Response, Problem> {
    if input.id.is_some()
        || input.version.is_some()
        || input.archived.is_some()
        || input.project_leader.is_some()
        || input.health_rating.is_some()
    {
        return Err(Problem::invalid(
            "id, archived, project_leader, health_rating and version are set by the server",
        ));
    }
    let Some(code) = input.project_code.as_deref() else {
        return Err(Problem::invalid("project_code is required"));
    };
    let project = create_project(
        &state.store,
        acting,
        NewProject {
            name: &input.name,
            description: input.description.flatten().as_deref(),
            project_code: code,
        },
    )
    .await?;
    let body = ProjectBody::from(get_project(&state.store, project.id()).await?);
    let location = format!("/api/v1/projects/{}", body.id);
    Ok((
        [(header::LOCATION, location)],
        with_etag(StatusCode::CREATED, body),
    )
        .into_response())
}

/// Fetches a Project, archived or not.
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    operation_id = "get_project",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = ProjectBody, headers(("etag" = String))),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
async fn fetch<S: Store>(
    State(state): State<AppState<S>>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    let view = get_project(&state.store, ProjectId::from_uuid(id)).await?;
    Ok(with_etag(StatusCode::OK, view.into()))
}

/// Replaces a Project's name, description, Health Rating and Project Leader.
/// Needs a session and `If-Match`.
#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    operation_id = "update_project",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body = ProjectBody,
    responses(
        (status = 200, body = ProjectBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Project", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn update<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<ProjectInput>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if input.version.is_some_and(|v| v != expected) {
        return Err(Problem::stale());
    }
    if input.id.is_some_and(|body_id| body_id != id) {
        return Err(Problem::invalid("id does not match the URL"));
    }
    let Some(description) = input.description else {
        return Err(Problem::invalid("description is required (null clears it)"));
    };
    let Some(leader) = input.project_leader else {
        return Err(Problem::invalid(
            "project_leader is required (null means no Project Leader)",
        ));
    };
    let Some(health_rating) = input.health_rating else {
        return Err(Problem::invalid(
            "health_rating is required (null means not rated)",
        ));
    };
    let id = ProjectId::from_uuid(id);
    let stored = get_project(&state.store, id).await?;
    if input
        .project_code
        .is_some_and(|code| code != stored.project_code)
    {
        return Err(Problem::invalid("project_code never changes"));
    }
    if input.archived.is_some_and(|a| a != stored.archived) {
        return Err(Problem::invalid("archived is changed only with PATCH"));
    }
    update_project(
        &state.store,
        acting,
        id,
        expected,
        ProjectEdit {
            name: &input.name,
            description: description.as_deref(),
            health_rating: health_rating.as_ref().map(|h| HealthRatingInput {
                rating: h.rating.into(),
                comment: h.comment.as_deref(),
            }),
            project_leader: leader.map(|l| PersonId::from_uuid(l.id)),
        },
    )
    .await?;
    Ok(with_etag(
        StatusCode::OK,
        get_project(&state.store, id).await?.into(),
    ))
}

/// Archives or unarchives a Project, with `{"archived": …}` as
/// `application/merge-patch+json`. Needs a session and `If-Match`.
#[utoipa::path(
    patch,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    operation_id = "archive_project",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body(content = ProjectPatch, content_type = "application/merge-patch+json"),
    responses(
        (status = 200, body = ProjectBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid patch", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn archive<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(patch): JsonBody<ProjectPatch>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = ProjectId::from_uuid(id);
    set_archived(&state.store, acting, id, expected, patch.archived).await?;
    Ok(with_etag(
        StatusCode::OK,
        get_project(&state.store, id).await?.into(),
    ))
}
