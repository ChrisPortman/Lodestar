//! `/api/v1/meetings` (ADR-0006): end, list and read Meetings. A Meeting is
//! immutable once created, so `PUT`, `PATCH` and `DELETE` are 405.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use lodestar_core::{
    meeting::use_cases::{NewMeeting, create_meeting, get_meeting, list_meetings},
    meeting::{MeetingId, MeetingSummary, MeetingView},
    minutes::render,
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
        .route(
            "/meetings/{id}/minutes",
            get(minutes::<S>)
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

/// The formats Minutes are served in, in order of preference on a tie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MinutesFormat {
    Json,
    Text,
    Html,
}

impl MinutesFormat {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Json, "application/json"),
        (Self::Text, "text/plain"),
        (Self::Html, "text/html"),
    ];

    /// Picks the format the `Accept` header rates highest; JSON when absent.
    fn negotiate(accept: Option<&str>) -> Option<Self> {
        let Some(accept) = accept.filter(|a| !a.trim().is_empty()) else {
            return Some(Self::Json);
        };
        let ranges: Vec<(String, f32)> = accept
            .split(',')
            .map(|range| {
                let mut parts = range.split(';');
                let media = parts.next().unwrap_or("").trim().to_ascii_lowercase();
                let q = parts
                    .filter_map(|p| p.trim().strip_prefix("q="))
                    .find_map(|q| q.trim().parse::<f32>().ok())
                    .unwrap_or(1.0);
                (media, q)
            })
            .collect();
        let quality = |media: &str| -> f32 {
            let (kind, _) = media.split_once('/').unwrap_or((media, ""));
            let wildcard = format!("{kind}/*");
            // The most specific matching range decides (RFC 9110 section 12.5.1).
            [media, wildcard.as_str(), "*/*"]
                .iter()
                .find_map(|want| ranges.iter().find(|(m, _)| m == want).map(|(_, q)| *q))
                .unwrap_or(0.0)
        };
        let mut best: Option<(Self, f32)> = None;
        for (format, media) in Self::ALL {
            let q = quality(media);
            if q > 0.0 && best.is_none_or(|(_, b)| q > b) {
                best = Some((format, q));
            }
        }
        best.map(|(format, _)| format)
    }
}

/// The reader's time zone for rendered Minutes; UTC when absent.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MinutesParams {
    /// Minutes east of UTC at the Meeting's start, e.g. 660 for AEDT.
    #[param(minimum = -840, maximum = 840)]
    tz_offset: Option<i32>,
    /// A short zone name to print, e.g. `AEDT`; `UTC±hh:mm` when absent.
    #[param(max_length = 32)]
    tz_name: Option<String>,
}

impl MinutesParams {
    fn zone(&self) -> Result<render::Zone, Problem> {
        match (self.tz_offset, self.tz_name.as_deref()) {
            (None, None) => Ok(render::Zone::utc()),
            (offset, name) => render::Zone::new(offset.unwrap_or(0), name.unwrap_or(""))
                .ok_or_else(|| {
                    Problem::new(StatusCode::BAD_REQUEST).detail(
                        "tz_offset must be within ±840 minutes and tz_name at most 32 characters",
                    )
                }),
        }
    }
}

/// Fetches a Meeting's Minutes as the stored JSON document, or rendered as
/// plain text or inline-styled HTML, by `Accept`. Rendered times are in the
/// zone `tz_offset`/`tz_name` give, else UTC. Any Member may.
#[utoipa::path(
    get,
    path = "/api/v1/meetings/{id}/minutes",
    tag = "meetings",
    operation_id = "get_meeting_minutes",
    params(("id" = Uuid, Path), MinutesParams),
    responses(
        (status = 200, description = "The Minutes", content(
            (Object = "application/json"),
            (String = "text/plain"),
            (String = "text/html"),
        )),
        (status = 400, description = "The zone is out of range", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a Member", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 406, description = "None of the accepted types is offered", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn minutes<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    Query(params): Query<MinutesParams>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let accept = headers
        .get(header::ACCEPT)
        .map(|v| v.to_str().unwrap_or(""));
    let format = MinutesFormat::negotiate(accept).ok_or_else(|| {
        Problem::new(StatusCode::NOT_ACCEPTABLE)
            .detail("minutes are offered as application/json, text/plain or text/html")
    })?;
    let zone = params.zone()?;
    let minutes = get_meeting(&state.store, acting, MeetingId::from_uuid(id))
        .await?
        .minutes;
    let vary = [(header::VARY, "accept")];
    Ok(match format {
        MinutesFormat::Json => (vary, Json(minutes)).into_response(),
        MinutesFormat::Text => (
            vary,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            render::to_text(&minutes, &zone),
        )
            .into_response(),
        MinutesFormat::Html => (
            vary,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            render::to_html(&minutes, &zone),
        )
            .into_response(),
    })
}

#[cfg(test)]
mod tests {
    use super::MinutesFormat::{self, *};

    #[test]
    fn accept_negotiation() {
        let n = MinutesFormat::negotiate;
        assert_eq!(n(None), Some(Json));
        assert_eq!(n(Some("text/plain")), Some(Text));
        assert_eq!(n(Some("TEXT/HTML; charset=utf-8")), Some(Html));
        assert_eq!(n(Some("text/*")), Some(Text));
        assert_eq!(n(Some("text/html;q=0.9, text/plain;q=0.8")), Some(Html));
        assert_eq!(n(Some("*/*, application/json;q=0")), Some(Text));
        assert_eq!(n(Some("image/png")), None);
    }
}
