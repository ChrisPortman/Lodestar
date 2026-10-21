//! `/api/v1/meetings` (ADR-0006): end, list and read Meetings. A Meeting is
//! immutable once created, so `PUT`, `PATCH` and `DELETE` are 405.

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use lodestar_core::{
    meeting::use_cases::{NewMeeting, create_meeting, get_meeting, list_meetings},
    meeting::{MeetingId, MeetingSummary, MeetingView},
    person::PersonId,
    project::ProjectId,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState, Store,
    problem::Problem,
    refs::PersonRefBody,
    request::{JsonBody, Path, Query, page_request},
    session::{Acting, ActingSession},
};

fn immutable(allow: &'static str) -> impl Fn() -> std::future::Ready<Problem> + Clone {
    move || {
        std::future::ready(
            Problem::method_not_allowed(allow).detail("meetings are never changed (ADR-0006)"),
        )
    }
}

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new()
        .route(
            "/meetings",
            get(list::<S>)
                .post(create::<S>)
                .put(immutable("GET,HEAD,POST"))
                .patch(immutable("GET,HEAD,POST"))
                .delete(immutable("GET,HEAD,POST")),
        )
        .route(
            "/meetings/{id}",
            get(fetch::<S>)
                .put(immutable("GET,HEAD"))
                .patch(immutable("GET,HEAD"))
                .delete(immutable("GET,HEAD")),
        )
}

/// A Meeting as listed: everything but its Minutes.
#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingSummaryBody {
    pub id: Uuid,
    pub project_id: Uuid,
    pub leader: PersonRefBody,
    /// The leader first, then the others in the order given.
    pub attendees: Vec<PersonRefBody>,
    #[schema(value_type = String, format = DateTime)]
    pub started_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub ended_at: DateTime<Utc>,
    #[schema(minimum = 1)]
    pub version: u32,
}

impl From<MeetingSummary> for MeetingSummaryBody {
    fn from(m: MeetingSummary) -> Self {
        Self {
            id: m.id.as_uuid(),
            project_id: m.project_id.as_uuid(),
            leader: m.leader.into(),
            attendees: m.attendees.into_iter().map(Into::into).collect(),
            started_at: m.started_at,
            ended_at: m.ended_at,
            version: m.version,
        }
    }
}

/// A Meeting with its frozen Minutes.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Meeting)]
pub struct MeetingBody {
    #[serde(flatten)]
    #[schema(inline)]
    pub summary: MeetingSummaryBody,
    /// The fixed, fully resolved Minutes document (`schema` gives its version).
    #[schema(value_type = Object)]
    pub minutes: serde_json::Value,
}

impl From<MeetingView> for MeetingBody {
    fn from(view: MeetingView) -> Self {
        Self {
            summary: view.summary.into(),
            minutes: serde_json::to_value(view.minutes).expect("minutes serialise to JSON"),
        }
    }
}

/// A Comment about the Meeting, made while it ran.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MeetingCommentInput {
    #[schema(max_length = 4000)]
    pub body: String,
}

/// Ends the Meeting the calling session holds the lock for.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewMeetingBody {
    pub project_id: Uuid,
    /// Members who attended; not empty. The leader is added if missing.
    pub attendee_ids: Vec<Uuid>,
    #[serde(default)]
    pub comments: Vec<MeetingCommentInput>,
}

/// A page of Meetings, newest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingPage {
    pub items: Vec<MeetingSummaryBody>,
    /// Meetings across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// The Project whose Meetings to list.
    project_id: Uuid,
    /// Page size, 1–1000.
    #[param(minimum = 1, maximum = 1000, default = 50)]
    limit: Option<u32>,
    #[param(minimum = 0, default = 0)]
    offset: Option<u32>,
}

/// Ends a Meeting: creates it over the Meeting Lock's time range with its
/// Comments and Minutes, and releases the lock, all at once.
#[utoipa::path(
    post,
    path = "/api/v1/meetings",
    tag = "meetings",
    operation_id = "create_meeting",
    request_body = NewMeetingBody,
    responses(
        (status = 201, body = MeetingBody, headers(("location" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not the Project Leader", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "This session does not hold the lock, or the Project is archived", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Meeting", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn create<S: Store>(
    State(state): State<AppState<S>>,
    ActingSession { person, session }: ActingSession,
    JsonBody(input): JsonBody<NewMeetingBody>,
) -> Result<Response, Problem> {
    let attendees: Vec<PersonId> = input
        .attendee_ids
        .into_iter()
        .map(PersonId::from_uuid)
        .collect();
    let comments: Vec<&str> = input.comments.iter().map(|c| c.body.as_str()).collect();
    let meeting = create_meeting(
        &state.store,
        person,
        &session,
        NewMeeting {
            project_id: ProjectId::from_uuid(input.project_id),
            attendee_ids: &attendees,
            comments: &comments,
        },
        Utc::now(),
    )
    .await?;
    let body = MeetingBody::from(get_meeting(&state.store, person, meeting.id()).await?);
    let location = format!("/api/v1/meetings/{}", body.summary.id);
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(body),
    )
        .into_response())
}

/// Lists a Project's Meetings newest first, without Minutes. Any Member may.
#[utoipa::path(
    get,
    path = "/api/v1/meetings",
    tag = "meetings",
    operation_id = "list_meetings",
    params(ListParams),
    responses(
        (status = 200, body = MeetingPage),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a Member", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Query(params): Query<ListParams>,
) -> Result<Json<MeetingPage>, Problem> {
    let page = page_request(params.limit, params.offset)?;
    let found = list_meetings(
        &state.store,
        acting,
        ProjectId::from_uuid(params.project_id),
        page,
    )
    .await?;
    Ok(Json(MeetingPage {
        items: found.items.into_iter().map(Into::into).collect(),
        total: found.total,
        limit: page.limit,
        offset: page.offset,
    }))
}

/// Fetches a Meeting with its Minutes. Any Member may.
#[utoipa::path(
    get,
    path = "/api/v1/meetings/{id}",
    tag = "meetings",
    operation_id = "get_meeting",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, body = MeetingBody),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a Member", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn fetch<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingBody>, Problem> {
    let view = get_meeting(&state.store, acting, MeetingId::from_uuid(id)).await?;
    Ok(Json(view.into()))
}
