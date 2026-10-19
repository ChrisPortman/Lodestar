//! `/api/v1/projects/{id}/export`: the JSON Project Export (FR-110, #15).

use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use lodestar_core::{
    action::use_cases::{ActionFilter, list_actions},
    blocker::use_cases::{BlockerFilter, list_blockers},
    history::HistoryEntry,
    person::use_cases::get_person,
    ports::{ClearedFilter, ClosedFilter, PageRequest, RiskFilter, RiskSort},
    project::{ProjectId, members::project_members, use_cases::get_project},
    risk::use_cases::list_risks,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState, Store,
    actions::ActionBody,
    blockers::BlockerBody,
    people::PersonBody,
    problem::Problem,
    projects::ProjectBody,
    refs::PersonRefBody,
    request::{Path, Query},
    risks::RiskBody,
    session::Acting,
};

/// The value of `format`.
pub const FORMAT: &str = "lodestar.project-export";
/// Bumped by hand whenever a change to a reused DTO alters the export.
pub const FORMAT_VERSION: u32 = 1;

/// Pages are fetched this size until every item is read.
const PAGE: u32 = 1000;

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new().route("/projects/{id}/export", get(export::<S>))
}

/// One Change History entry, with its snapshots exactly as stored. Snapshots
/// follow the domain shape when the entry was written, not `format_version`.
#[derive(Debug, Serialize, ToSchema)]
pub struct ChangeHistoryEntry {
    pub id: Uuid,
    /// `project`, `action`, `risk` or `blocker`.
    pub entity_type: String,
    pub entity_id: Uuid,
    /// The entity version this change produced.
    pub version: u32,
    /// `created` or `updated`.
    pub change_kind: String,
    /// Listed in `people`.
    pub acting_person_id: Uuid,
    #[schema(value_type = String, format = DateTime)]
    pub recorded_at: DateTime<Utc>,
    /// `null` on creation.
    #[schema(value_type = Option<Object>)]
    pub before: Option<serde_json::Value>,
    #[schema(value_type = Object)]
    pub after: serde_json::Value,
    /// The entry that triggered this one, if any.
    pub caused_by: Option<Uuid>,
}

impl From<HistoryEntry> for ChangeHistoryEntry {
    fn from(e: HistoryEntry) -> Self {
        Self {
            id: e.id.as_uuid(),
            entity_type: e.entity_type.as_str().to_owned(),
            entity_id: e.entity_id,
            version: e.version,
            change_kind: e.change_kind.as_str().to_owned(),
            acting_person_id: e.acting_person.as_uuid(),
            recorded_at: e.recorded_at,
            before: e.before,
            after: e.after,
            caused_by: e.caused_by.map(|c| c.as_uuid()),
        }
    }
}

/// A whole Project as one JSON document (FR-110).
#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectExport {
    /// Always `lodestar.project-export`.
    pub format: String,
    /// Currently 1.
    pub format_version: u32,
    #[schema(value_type = String, format = DateTime)]
    pub exported_at: DateTime<Utc>,
    /// The Acting Person.
    pub exported_by: PersonRefBody,
    pub project: ProjectBody,
    /// Ordered by display name.
    pub members: Vec<PersonRefBody>,
    /// Every Action, closed ones included, ordered by Reference.
    pub actions: Vec<ActionBody>,
    /// Every Risk, closed ones included, ordered by Reference.
    pub risks: Vec<RiskBody>,
    /// Every Blocker, cleared ones included, ordered by Reference.
    pub blockers: Vec<BlockerBody>,
    /// Every Person the document mentions, ordered by display name.
    pub people: Vec<PersonBody>,
    /// Present only with `include_history=true`: every entry for the
    /// Project and its items, in the order recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_history: Option<Vec<ChangeHistoryEntry>>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExportParams {
    /// Include the Change History.
    #[param(default = false)]
    include_history: Option<bool>,
}

/// Downloads a Project, archived or not, with all its Members, Actions, Risks
/// and Blockers as an attachment named `<project-code>-<date>.json`. Needs a
/// session.
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/export",
    tag = "projects",
    operation_id = "export_project",
    params(("id" = Uuid, Path), ExportParams),
    responses(
        (status = 200, body = ProjectExport, headers(("content-disposition" = String))),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
        (status = 404, body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid parameters", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn export<S: Store>(
    State(state): State<AppState<S>>,
    Acting(acting): Acting,
    Path(id): Path<Uuid>,
    Query(params): Query<ExportParams>,
) -> Result<Response, Problem> {
    let project_id = ProjectId::from_uuid(id);
    let project = get_project(&state.store, project_id).await?;
    let exported_by = get_person(&state.store, acting).await?;
    let members = project_members(&state.store, project_id).await?;
    let actions = all(|page| {
        list_actions(
            &state.store,
            ActionFilter {
                project_id: Some(project_id),
                state: ClosedFilter::All,
                ..ActionFilter::default()
            },
            page,
        )
    })
    .await?;
    let risks = all(|page| {
        list_risks(
            &state.store,
            page,
            RiskFilter {
                project_id: Some(project_id),
                state: ClosedFilter::All,
                ..RiskFilter::default()
            },
            RiskSort::Reference,
        )
    })
    .await?;
    let blockers = all(|page| {
        list_blockers(
            &state.store,
            BlockerFilter {
                project_id: Some(project_id),
                state: ClearedFilter::All,
                ..BlockerFilter::default()
            },
            page,
        )
    })
    .await?;
    let history = if params.include_history.unwrap_or(false) {
        Some(state.store.project_history(project_id).await?)
    } else {
        None
    };

    let mut mentioned = HashSet::new();
    mentioned.insert(acting);
    mentioned.extend(project.project_leader.iter().map(|l| l.id));
    mentioned.extend(members.iter().map(|m| m.id));
    mentioned.extend(
        actions
            .iter()
            .filter_map(|a| a.assignee.as_ref().map(|p| p.id)),
    );
    mentioned.extend(
        blockers
            .iter()
            .filter_map(|b| b.accountable.as_ref().map(|p| p.id)),
    );
    mentioned.extend(history.iter().flatten().map(|e| e.acting_person));
    let mut people = Vec::with_capacity(mentioned.len());
    for person in mentioned {
        people.push(PersonBody::from(get_person(&state.store, person).await?));
    }
    people.sort_by(|a, b| a.display_name.cmp(&b.display_name).then(a.id.cmp(&b.id)));

    let exported_at = Utc::now();
    let filename = format!("{}-{}.json", project.project_code, exported_at.date_naive());
    let body = ProjectExport {
        format: FORMAT.to_owned(),
        format_version: FORMAT_VERSION,
        exported_at,
        exported_by: PersonRefBody {
            id: exported_by.id.as_uuid(),
            display_name: exported_by.display_name,
        },
        project: project.into(),
        members: members.into_iter().map(PersonRefBody::from).collect(),
        actions: actions.into_iter().map(ActionBody::from).collect(),
        risks: risks.into_iter().map(RiskBody::from).collect(),
        blockers: blockers.into_iter().map(BlockerBody::from).collect(),
        people,
        change_history: history.map(|h| h.into_iter().map(ChangeHistoryEntry::from).collect()),
    };
    Ok((
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )],
        Json(body),
    )
        .into_response())
}

/// Reads every page of a listing.
async fn all<T, E, F, Fut>(mut list: F) -> Result<Vec<T>, E>
where
    F: FnMut(PageRequest) -> Fut,
    Fut: Future<Output = Result<lodestar_core::ports::Page<T>, E>>,
{
    let mut items = Vec::new();
    loop {
        let page = list(PageRequest {
            offset: u32::try_from(items.len()).unwrap_or(u32::MAX),
            limit: PAGE,
        })
        .await?;
        let done = page.items.len() < PAGE as usize;
        items.extend(page.items);
        if done || items.len() as u64 >= page.total {
            return Ok(items);
        }
    }
}
