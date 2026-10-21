//! Meeting use cases (ADR-0006). Ending a Meeting runs in one unit of work:
//! the Meeting, its Comments and its Minutes are stored and the Meeting Lock
//! is released together, or not at all.

use chrono::{DateTime, SubsecRound, Utc};

use crate::comment::{BodyError, Comment, CommentBody, Target, TargetType};
use crate::history::ChangeContext;
use crate::meeting::{Meeting, MeetingId, MeetingSummary, MeetingView, attendees_led_by};
use crate::minutes::{self, MinutesHeader};
use crate::person::PersonId;
use crate::ports::{
    ActionsQuery, BlockersQuery, CommentRepository, HistoryQuery, MeetingLockRepository,
    MeetingRepository, MeetingsQuery, Page, PageRequest, PeopleQuery, PersonRepository,
    ProjectsQuery, RepoError, RisksQuery, Transaction, UnitOfWork,
};
use crate::project::ProjectId;
use crate::project::guard::{GuardError, writable_project};
use crate::session::SessionId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MeetingError {
    #[error(transparent)]
    InvalidComment(#[from] BodyError),
    #[error("a meeting needs at least one attendee")]
    NoAttendees,
    #[error("every attendee must be a member of the project")]
    AttendeeNotAMember,
    #[error("the project does not exist")]
    UnknownProject,
    #[error("only the project leader can end a meeting")]
    NotTheLeader,
    #[error("only members of the project can see its meetings")]
    NotAMember,
    #[error("this session does not hold the meeting lock")]
    NotHeld,
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl From<GuardError> for MeetingError {
    fn from(error: GuardError) -> Self {
        match error {
            GuardError::Archived => Self::Archived,
            GuardError::Locked(_) => Self::NotHeld,
            GuardError::Repo(RepoError::NotFound) => Self::UnknownProject,
            GuardError::Repo(e) => Self::Repo(e),
        }
    }
}

/// What a Meeting is created from when its leader ends it.
#[derive(Debug, Clone, Copy)]
pub struct NewMeeting<'a> {
    pub project_id: ProjectId,
    /// Must not be empty; the leader is added if missing.
    pub attendee_ids: &'a [PersonId],
    /// Comments about the Meeting itself.
    pub comments: &'a [&'a str],
}

/// Everything ending a Meeting reads: the unit of work, plus Change History
/// and the names its Minutes resolve.
pub trait MeetingStore:
    UnitOfWork + HistoryQuery + PeopleQuery + ActionsQuery + RisksQuery + BlockersQuery
{
}

impl<T> MeetingStore for T where
    T: UnitOfWork + HistoryQuery + PeopleQuery + ActionsQuery + RisksQuery + BlockersQuery
{
}

/// Ends the Meeting the calling `session` holds the lock for (its own stale
/// lock included): creates the Meeting over the lock's time range, its
/// Comments, and its Minutes, and releases the lock.
pub async fn create_meeting<S: MeetingStore>(
    store: &S,
    acting: PersonId,
    session: &SessionId,
    input: NewMeeting<'_>,
    now: DateTime<Utc>,
) -> Result<Meeting, MeetingError> {
    let bodies = input
        .comments
        .iter()
        .map(|b| CommentBody::new(*b))
        .collect::<Result<Vec<_>, _>>()?;
    if input.attendee_ids.is_empty() {
        return Err(MeetingError::NoAttendees);
    }
    let mut tx = store.begin().await?;
    let project = writable_project(&mut tx, input.project_id).await?;
    let lock = match tx.meeting_locks().load(input.project_id).await? {
        Some(lock) if lock.is_held_by(session) => lock,
        _ => return Err(MeetingError::NotHeld),
    };
    if project.project_leader() != Some(acting) {
        return Err(MeetingError::NotTheLeader);
    }
    let attendees = attendees_led_by(acting, input.attendee_ids);
    if !attendees.iter().all(|a| project.members().contains(a)) {
        return Err(MeetingError::AttendeeNotAMember);
    }

    let (start, end) = (
        lock.acquired_at(),
        now.trunc_subsecs(6).max(lock.acquired_at()),
    );
    let mut names = Vec::with_capacity(attendees.len());
    for &id in &attendees {
        names.push(
            tx.people()
                .load(id)
                .await?
                .display_name()
                .as_str()
                .to_owned(),
        );
    }
    let history = store
        .project_history_between(input.project_id, start, end)
        .await?;
    let lookups = minutes::resolve(store, &minutes::needed(&history)).await;
    let mut minutes = minutes::build_minutes(
        MinutesHeader {
            project_name: project.name().as_str().to_owned(),
            start,
            end,
            leader: names[0].clone(),
            attendees: names,
        },
        &history,
        &lookups,
    );
    minutes
        .meeting_comments
        .extend(bodies.iter().map(|b| b.as_str().to_owned()));

    let id = MeetingId::new();
    let meeting = Meeting::create(
        id,
        input.project_id,
        acting,
        &attendees,
        start,
        end,
        minutes,
    );
    let entry = tx
        .meetings()
        .create(&meeting, &ChangeContext::by(acting))
        .await?;
    let target = Target {
        kind: TargetType::Meeting,
        id: id.as_uuid(),
    };
    for body in bodies {
        let comment = Comment::create(input.project_id, target, acting, body);
        tx.comments()
            .save(None, &comment, &ChangeContext::by(acting).caused_by(entry))
            .await?;
    }
    tx.meeting_locks().delete(&lock).await?;
    tx.commit().await?;
    Ok(meeting)
}

async fn require_member<Q: ProjectsQuery>(
    query: &Q,
    acting: PersonId,
    project: ProjectId,
) -> Result<(), MeetingError> {
    match query.project_members(project).await {
        Ok(members) if members.iter().any(|m| m.id == acting) => Ok(()),
        Ok(_) => Err(MeetingError::NotAMember),
        Err(RepoError::NotFound) => Err(MeetingError::UnknownProject),
        Err(e) => Err(e.into()),
    }
}

/// A Project's Meetings, newest first. Any Member may list them.
pub async fn list_meetings<Q: MeetingsQuery + ProjectsQuery>(
    query: &Q,
    acting: PersonId,
    project: ProjectId,
    page: PageRequest,
) -> Result<Page<MeetingSummary>, MeetingError> {
    require_member(query, acting, project).await?;
    Ok(query.list_meetings(project, page).await?)
}

/// A Meeting with its Minutes. Any Member of its Project may read it.
pub async fn get_meeting<Q: MeetingsQuery + ProjectsQuery>(
    query: &Q,
    acting: PersonId,
    id: MeetingId,
) -> Result<MeetingView, MeetingError> {
    let view = query.get_meeting(id).await?;
    require_member(query, acting, view.summary.project_id).await?;
    Ok(view)
}
