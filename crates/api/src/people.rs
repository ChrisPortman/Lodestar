//! `/api/v1/people`: list, create, fetch and update People (FR-01, FR-05).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use lodestar_core::{
    person::{Person, PersonId, PersonView},
    ports::{PageRequest, PeopleSort},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState,
    problem::Problem,
    request::{JsonBody, Path, Query, etag, if_match},
    session::{Acting, MaybeActing, cookie_for},
};

pub const DEFAULT_LIMIT: u32 = 50;
pub const MAX_LIMIT: u32 = 1000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/people",
            get(list).post(create).delete(not_allowed("GET,HEAD,POST")),
        )
        .route(
            "/people/{id}",
            get(fetch).put(update).delete(not_allowed("GET,HEAD,PUT")),
        )
}

fn not_allowed(allow: &'static str) -> impl Fn() -> std::future::Ready<Problem> + Clone {
    move || std::future::ready(Problem::method_not_allowed(allow))
}

/// A Person. Also the request body for `POST` and `PUT`: read-only fields may
/// be sent only if they match what is stored.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = Person)]
pub struct PersonBody {
    #[schema(read_only)]
    pub id: Uuid,
    /// Required, 1–100 characters; surrounding whitespace is trimmed.
    #[schema(min_length = 1, max_length = 100)]
    pub display_name: String,
    #[schema(read_only, minimum = 1)]
    pub version: u32,
}

impl From<PersonView> for PersonBody {
    fn from(view: PersonView) -> Self {
        Self {
            id: view.id.as_uuid(),
            display_name: view.display_name,
            version: view.version,
        }
    }
}

impl From<&Person> for PersonBody {
    fn from(person: &Person) -> Self {
        Self {
            id: person.id().as_uuid(),
            display_name: person.display_name().as_str().to_owned(),
            version: person.version(),
        }
    }
}

/// What a client may send for a Person; documented as [`PersonBody`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersonInput {
    id: Option<Uuid>,
    display_name: String,
    version: Option<u32>,
}

/// A Person with its `ETag`.
fn with_etag(status: StatusCode, person: PersonBody) -> Response {
    (status, [(header::ETAG, etag(person.version))], Json(person)).into_response()
}

/// A page of People.
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonPage {
    pub items: Vec<PersonBody>,
    /// Matching People across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PersonSort {
    /// Creation order.
    #[default]
    Id,
    DisplayName,
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
    sort: Option<PersonSort>,
}

/// Lists People, ordered by ID unless `sort` is given.
#[utoipa::path(
    get,
    path = "/api/v1/people",
    tag = "people",
    operation_id = "list_people",
    params(ListParams),
    responses(
        (status = 200, body = PersonPage),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<PersonPage>, Problem> {
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(Problem::invalid(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let offset = params.offset.unwrap_or(0);
    let sort = match params.sort.unwrap_or_default() {
        PersonSort::Id => PeopleSort::Id,
        PersonSort::DisplayName => PeopleSort::DisplayName,
    };
    let page = state
        .people
        .list(PageRequest { offset, limit }, sort)
        .await?;
    Ok(Json(PersonPage {
        items: page.items.into_iter().map(PersonBody::from).collect(),
        total: page.total,
        limit,
        offset,
    }))
}

/// Creates a Person. Allowed without a session, so the first Person can be
/// created; they then become their own Acting Person.
#[utoipa::path(
    post,
    path = "/api/v1/people",
    tag = "people",
    operation_id = "create_person",
    request_body = PersonBody,
    responses(
        (status = 201, body = PersonBody, headers(
            ("location" = String), ("etag" = String),
            ("set-cookie" = String, description = "Set when there was no session"),
        )),
        (status = 422, description = "Invalid Person", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create(
    State(state): State<AppState>,
    MaybeActing(acting): MaybeActing,
    JsonBody(input): JsonBody<PersonInput>,
) -> Result<Response, Problem> {
    if input.id.is_some() || input.version.is_some() {
        return Err(Problem::invalid(
            "id and version are assigned by the server",
        ));
    }
    let person = state.people.create(acting, &input.display_name).await?;
    let location = format!("/api/v1/people/{}", person.id());
    let response = with_etag(StatusCode::CREATED, PersonBody::from(&person));
    let response = ([(header::LOCATION, location)], response);
    Ok(match acting {
        Some(_) => response.into_response(),
        None => (cookie_for(&state.session_key, person.id()), response).into_response(),
    })
}

/// Fetches a Person.
#[utoipa::path(
    get,
    path = "/api/v1/people/{id}",
    tag = "people",
    operation_id = "get_person",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = PersonBody, headers(("etag" = String))),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
async fn fetch(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let view = state.people.get(PersonId::from_uuid(id)).await?;
    Ok(with_etag(StatusCode::OK, view.into()))
}

/// Replaces a Person. Needs a session and `If-Match`.
#[utoipa::path(
    put,
    path = "/api/v1/people/{id}",
    tag = "people",
    operation_id = "update_person",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the version being changed")),
    request_body = PersonBody,
    responses(
        (status = 200, body = PersonBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Person", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn update(
    State(state): State<AppState>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<PersonInput>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    if input.version.is_some_and(|v| v != expected) {
        return Err(Problem::stale());
    }
    if input.id.is_some_and(|body_id| body_id != id) {
        return Err(Problem::invalid("id does not match the URL"));
    }
    let person = state
        .people
        .rename(
            acting,
            PersonId::from_uuid(id),
            expected,
            &input.display_name,
        )
        .await?;
    Ok(with_etag(StatusCode::OK, PersonBody::from(&person)))
}
