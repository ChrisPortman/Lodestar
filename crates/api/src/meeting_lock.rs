//! `/api/v1/projects/{id}/meeting-lock` (ADR-0006): a Project's Meeting
//! Lock, held by one browser session. `POST` acquires, `PUT` heartbeats,
//! `GET` reads and `DELETE` cancels. It is not a domain resource: no
//! `version`, no `ETag`, no Change History.

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use chrono::{DateTime, Utc};
use lodestar_core::{
    meeting_lock::use_cases::{LockStatus, acquire_lock, cancel_lock, heartbeat_lock, lock_status},
    person::PersonId,
    project::ProjectId,
    session::SessionId,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState, Store, problem::Problem, refs::PersonRefBody, request::Path, session::ActingSession,
};

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new().route(
        "/projects/{id}/meeting-lock",
        post(acquire::<S>)
            .put(heartbeat::<S>)
            .get(read::<S>)
            .delete(cancel::<S>),
    )
}

/// A Project's Meeting Lock, as the calling session sees it.
#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingLockBody {
    /// The Project Leader who acquired it.
    pub leader: PersonRefBody,
    #[schema(value_type = String, format = DateTime)]
    pub acquired_at: DateTime<Utc>,
    /// Whether it was heartbeaten in the last 2 minutes. A stale lock blocks
    /// nothing.
    pub live: bool,
    /// Whether the calling session holds it.
    pub held_by_you: bool,
}

impl From<LockStatus> for MeetingLockBody {
    fn from(status: LockStatus) -> Self {
        Self {
            leader: status.leader.into(),
            acquired_at: status.acquired_at,
            live: status.live,
            held_by_you: status.held_by_you,
        }
    }
}

async fn status<S: Store>(
    state: &AppState<S>,
    person: PersonId,
    session: &SessionId,
    project: ProjectId,
) -> Result<Json<MeetingLockBody>, Problem> {
    let status = lock_status(&state.store, person, session, project, Utc::now()).await?;
    Ok(Json(status.into()))
}

/// Acquires the Meeting Lock for this session. Only the Project Leader may.
/// A stale lock is taken over; this session's own lock is reclaimed.
#[utoipa::path(
    post,
    path = "/api/v1/projects/{id}/meeting-lock",
    tag = "meetings",
    operation_id = "acquire_meeting_lock",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = MeetingLockBody),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not the Project Leader", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Another session holds a live lock", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn acquire<S: Store>(
    State(state): State<AppState<S>>,
    ActingSession { person, session }: ActingSession,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingLockBody>, Problem> {
    let project = ProjectId::from_uuid(id);
    acquire_lock(&state.store, person, &session, project, Utc::now()).await?;
    status(&state, person, &session, project).await
}

/// Keeps this session's Meeting Lock live; send it well within 2 minutes.
#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}/meeting-lock",
    tag = "meetings",
    operation_id = "heartbeat_meeting_lock",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = MeetingLockBody),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "This session does not hold the lock", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn heartbeat<S: Store>(
    State(state): State<AppState<S>>,
    ActingSession { person, session }: ActingSession,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingLockBody>, Problem> {
    let project = ProjectId::from_uuid(id);
    heartbeat_lock(&state.store, &session, project, Utc::now()).await?;
    status(&state, person, &session, project).await
}

/// Reads a Project's Meeting Lock. Any Member may.
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/meeting-lock",
    tag = "meetings",
    operation_id = "get_meeting_lock",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = MeetingLockBody),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a Member", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such Project, or no lock", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn read<S: Store>(
    State(state): State<AppState<S>>,
    ActingSession { person, session }: ActingSession,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingLockBody>, Problem> {
    status(&state, person, &session, ProjectId::from_uuid(id)).await
}

/// Cancels this session's Meeting Lock, deleting it.
#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}/meeting-lock",
    tag = "meetings",
    operation_id = "cancel_meeting_lock",
    params(("id" = Uuid, Path)),
    responses(
        (status = 204, description = "Cancelled"),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "This session does not hold the lock", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn cancel<S: Store>(
    State(state): State<AppState<S>>,
    ActingSession { session, .. }: ActingSession,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Problem> {
    cancel_lock(&state.store, &session, ProjectId::from_uuid(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
