//! `/api/v1/projects/{id}/members`: list, add and remove Members (FR-20 to
//! FR-22). Members are part of the Project, so changes need the Project's
//! `If-Match` and return the Project with its new `ETag`.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{delete, get},
};
use lodestar_core::{
    person::PersonId,
    project::{
        ProjectId,
        members::{add_member, project_members, remove_member},
        use_cases::get_project,
    },
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState, Store,
    problem::Problem,
    projects::{ProjectBody, with_etag},
    refs::PersonRefBody,
    request::{JsonBody, Path, if_match},
    session::Acting,
};

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new()
        .route("/projects/{id}/members", get(list::<S>).post(add::<S>))
        .route("/projects/{id}/members/{person_id}", delete(remove::<S>))
}

/// A Project's Members, ordered by display name.
#[derive(Debug, Serialize, ToSchema)]
pub struct MemberList {
    pub items: Vec<PersonRefBody>,
}

/// The Person to add as a Member.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewMember {
    pub person_id: Uuid,
}

/// Lists a Project's Members, archived Project or not.
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/members",
    tag = "projects",
    operation_id = "list_members",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = MemberList),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list<S: Store>(
    State(state): State<AppState<S>>,
    Path(id): Path<Uuid>,
) -> Result<Json<MemberList>, Problem> {
    let members = project_members(&state.store, ProjectId::from_uuid(id)).await?;
    Ok(Json(MemberList {
        items: members.into_iter().map(PersonRefBody::from).collect(),
    }))
}

/// Adds a Person as a Member. Adding an existing Member changes nothing; a
/// Person added back is not reassigned their old items. Needs a session and
/// the Project's `If-Match`.
#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/members",
    tag = "projects",
    operation_id = "add_member",
    params(("id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the Project version being changed")),
    request_body = NewMember,
    responses(
        (status = 200, body = ProjectBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "No such Person", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn add<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonBody(input): JsonBody<NewMember>,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = ProjectId::from_uuid(id);
    add_member(
        &state.store,
        acting,
        id,
        expected,
        PersonId::from_uuid(input.person_id),
    )
    .await?;
    Ok(with_etag(
        StatusCode::OK,
        get_project(&state.store, id).await?.into(),
    ))
}

/// Removes a Member. Their open Actions and Blockers become unassigned, and
/// removing the Project Leader leaves no Project Leader. Needs a session and
/// the Project's `If-Match`.
#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}/members/{person_id}",
    tag = "projects",
    operation_id = "remove_member",
    params(("id" = Uuid, Path), ("person_id" = Uuid, Path), ("if-match" = String, Header, description = "ETag of the Project version being changed")),
    responses(
        (status = 200, body = ProjectBody, headers(("etag" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such Project, or not a Member", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Out of date", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "If-Match missing", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn remove<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path((id, person_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let expected = if_match(&headers)?;
    let id = ProjectId::from_uuid(id);
    remove_member(
        &state.store,
        acting,
        id,
        expected,
        PersonId::from_uuid(person_id),
    )
    .await?;
    Ok(with_etag(
        StatusCode::OK,
        get_project(&state.store, id).await?.into(),
    ))
}
