//! Unit of work, repository and query ports (ADR-0007).
//!
//! Adapters live in `lodestar-datarepo`; in-memory fakes live in
//! [`crate::test_support`]. No adapter types appear here.

use std::collections::HashSet;
use std::future::Future;

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use crate::action::{Action, ActionId, ActionView};
use crate::blocker::{Blocker, BlockerId, BlockerView};
use crate::comment::{Comment, CommentId, CommentView, Target};
use crate::history::{ChangeContext, EntityType, HistoryEntry, HistoryEntryId};
use crate::meeting::{Meeting, MeetingId, MeetingSummary, MeetingView};
use crate::meeting_lock::MeetingLock;
use crate::person::{Person, PersonId, PersonView};
use crate::project::{PersonRef, Project, ProjectId, ProjectView};
use crate::risk::{Risk, RiskId, RiskView};

/// Errors every port can return. Adapters map their own errors onto these.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoError {
    #[error("not found")]
    NotFound,
    /// The stored version no longer matches the version the change was based on.
    #[error("conflicting change")]
    Conflict,
    #[error("unique constraint violated: {constraint}")]
    UniqueViolation { constraint: String },
    #[error("unexpected storage error: {0}")]
    Unexpected(String),
}

/// Starts transactions. Each use case runs in exactly one.
pub trait UnitOfWork: Send + Sync {
    type Tx: Transaction;

    fn begin(&self) -> impl Future<Output = Result<Self::Tx, RepoError>> + Send;
}

/// An open transaction. Dropping it without [`Transaction::commit`] rolls back.
pub trait Transaction: Send {
    fn people(&mut self) -> impl PersonRepository + Send + '_;

    fn projects(&mut self) -> impl ProjectRepository + Send + '_;

    fn references(&mut self) -> impl ReferenceCounter + Send + '_;

    fn risks(&mut self) -> impl RiskRepository + Send + '_;

    fn actions(&mut self) -> impl ActionRepository + Send + '_;

    fn blockers(&mut self) -> impl BlockerRepository + Send + '_;

    fn comments(&mut self) -> impl CommentRepository + Send + '_;

    fn meeting_locks(&mut self) -> impl MeetingLockRepository + Send + '_;

    fn meetings(&mut self) -> impl MeetingRepository + Send + '_;

    fn commit(self) -> impl Future<Output = Result<(), RepoError>> + Send;
}

pub trait PersonRepository {
    fn load(&mut self, id: PersonId) -> impl Future<Output = Result<Person, RepoError>> + Send;

    /// Stores `after`, which must be one version past `before` (or version 1
    /// when `before` is `None`), and writes its Change History entry.
    /// Returns [`RepoError::Conflict`] if the stored version is not `before`'s.
    fn save(
        &mut self,
        before: Option<&Person>,
        after: &Person,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;
}

pub trait ProjectRepository {
    fn load(&mut self, id: ProjectId) -> impl Future<Output = Result<Project, RepoError>> + Send;

    /// As [`PersonRepository::save`]. Creating a Project whose code is in use
    /// is a [`RepoError::UniqueViolation`].
    fn save(
        &mut self,
        before: Option<&Project>,
        after: &Project,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;
}

pub trait ActionRepository {
    fn load(&mut self, id: ActionId) -> impl Future<Output = Result<Action, RepoError>> + Send;

    /// As [`PersonRepository::save`].
    fn save(
        &mut self,
        before: Option<&Action>,
        after: &Action,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;

    /// The open Actions in `project` assigned to `person`.
    fn open_assigned_to(
        &mut self,
        project: ProjectId,
        person: PersonId,
    ) -> impl Future<Output = Result<Vec<Action>, RepoError>> + Send;
}

pub trait BlockerRepository {
    fn load(&mut self, id: BlockerId) -> impl Future<Output = Result<Blocker, RepoError>> + Send;

    /// As [`PersonRepository::save`], including the Blocker's Action links.
    fn save(
        &mut self,
        before: Option<&Blocker>,
        after: &Blocker,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;

    /// The open Blockers in `project` that `person` is accountable for.
    fn open_accountable(
        &mut self,
        project: ProjectId,
        person: PersonId,
    ) -> impl Future<Output = Result<Vec<Blocker>, RepoError>> + Send;
}

pub trait RiskRepository {
    fn load(&mut self, id: RiskId) -> impl Future<Output = Result<Risk, RepoError>> + Send;

    /// As [`PersonRepository::save`].
    fn save(
        &mut self,
        before: Option<&Risk>,
        after: &Risk,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;
}

pub trait CommentRepository {
    fn load(&mut self, id: CommentId) -> impl Future<Output = Result<Comment, RepoError>> + Send;

    /// As [`PersonRepository::save`].
    fn save(
        &mut self,
        before: Option<&Comment>,
        after: &Comment,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;
}

/// Meetings. They are only ever created, so there is no `before`.
pub trait MeetingRepository {
    fn load(&mut self, id: MeetingId) -> impl Future<Output = Result<Meeting, RepoError>> + Send;

    /// Stores a new Meeting and writes its Change History entry.
    fn create(
        &mut self,
        meeting: &Meeting,
        ctx: &ChangeContext,
    ) -> impl Future<Output = Result<HistoryEntryId, RepoError>> + Send;
}

/// Meeting Locks, at most one per Project. Not a domain resource: no
/// version and no Change History.
pub trait MeetingLockRepository {
    fn load(
        &mut self,
        project: ProjectId,
    ) -> impl Future<Output = Result<Option<MeetingLock>, RepoError>> + Send;

    /// Stores `after` in place of `before` (`None`: no lock). Returns
    /// [`RepoError::Conflict`] if the stored lock is no longer `before`.
    fn save(
        &mut self,
        before: Option<&MeetingLock>,
        after: &MeetingLock,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    /// Deletes `lock`; [`RepoError::Conflict`] if it is no longer stored.
    fn delete(&mut self, lock: &MeetingLock) -> impl Future<Output = Result<(), RepoError>> + Send;
}

/// The kinds of item that get a per-Project Reference (ADR-0001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReferenceKind {
    Action,
    Risk,
    Blocker,
}

impl ReferenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Risk => "risk",
            Self::Blocker => "blocker",
        }
    }

    /// The Reference for item `number` of this kind: `R-001`, …, `R-1000`.
    pub fn reference(self, number: u32) -> String {
        let prefix = match self {
            Self::Action => 'A',
            Self::Risk => 'R',
            Self::Blocker => 'B',
        };
        format!("{prefix}-{number:03}")
    }
}

/// Allocates Reference numbers, counting from 1 per `(project, kind)`.
pub trait ReferenceCounter {
    fn next(
        &mut self,
        project_id: Uuid,
        kind: ReferenceKind,
    ) -> impl Future<Output = Result<u32, RepoError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub offset: u32,
    pub limit: u32,
}

impl PageRequest {
    pub fn first(limit: u32) -> Self {
        Self { offset: 0, limit }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Total matching items across all pages.
    pub total: u64,
}

/// The order People are listed in. Ties are broken by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PeopleSort {
    /// Creation order (IDs are UUIDv7).
    #[default]
    Id,
    DisplayName,
}

/// Read-only views of People.
pub trait PeopleQuery: Send + Sync {
    fn list_people(
        &self,
        page: PageRequest,
        sort: PeopleSort,
    ) -> impl Future<Output = Result<Page<PersonView>, RepoError>> + Send;

    fn get_person(
        &self,
        id: PersonId,
    ) -> impl Future<Output = Result<PersonView, RepoError>> + Send;
}

/// The order Projects are listed in. Ties are broken by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSort {
    /// Creation order (IDs are UUIDv7).
    #[default]
    Id,
    Name,
}

/// Read-only views of Projects.
pub trait ProjectsQuery: Send + Sync {
    /// Lists Projects, leaving out archived ones unless `include_archived`.
    fn list_projects(
        &self,
        page: PageRequest,
        include_archived: bool,
        sort: ProjectSort,
    ) -> impl Future<Output = Result<Page<ProjectView>, RepoError>> + Send;

    fn get_project(
        &self,
        id: ProjectId,
    ) -> impl Future<Output = Result<ProjectView, RepoError>> + Send;

    /// A Project's Members, ordered by display name then ID.
    /// [`RepoError::NotFound`] if there is no such Project.
    fn project_members(
        &self,
        id: ProjectId,
    ) -> impl Future<Output = Result<Vec<PersonRef>, RepoError>> + Send;

    /// Every Project Code in use that starts with `prefix`.
    fn project_codes_starting_with(
        &self,
        prefix: &str,
    ) -> impl Future<Output = Result<HashSet<String>, RepoError>> + Send;
}

/// The order Risks are listed in. Ties are broken by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RiskSort {
    /// Creation order (IDs are UUIDv7).
    #[default]
    Id,
    /// Reference number, within each Project.
    Reference,
    /// Highest score first (FR-41).
    Score,
}

/// Which Actions or Risks to list by whether they are closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClosedFilter {
    #[default]
    Open,
    Closed,
    All,
}

impl ClosedFilter {
    /// Whether an item that is or isn't `closed` passes this filter.
    #[must_use]
    pub fn matches(self, closed: bool) -> bool {
        match self {
            Self::Open => !closed,
            Self::Closed => closed,
            Self::All => true,
        }
    }
}

/// Which Blockers to list by whether they are cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClearedFilter {
    #[default]
    Open,
    Cleared,
    All,
}

impl ClearedFilter {
    /// Whether a Blocker that is or isn't `cleared` passes this filter.
    #[must_use]
    pub fn matches(self, cleared: bool) -> bool {
        match self {
            Self::Open => !cleared,
            Self::Cleared => cleared,
            Self::All => true,
        }
    }
}

/// Which Risks to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RiskFilter {
    /// Only this Project's Risks; every Project's when `None`.
    pub project_id: Option<ProjectId>,
    pub state: ClosedFilter,
}

/// Read-only views of Risks.
pub trait RisksQuery: Send + Sync {
    fn list_risks(
        &self,
        page: PageRequest,
        filter: RiskFilter,
        sort: RiskSort,
    ) -> impl Future<Output = Result<Page<RiskView>, RepoError>> + Send;

    fn get_risk(&self, id: RiskId) -> impl Future<Output = Result<RiskView, RepoError>> + Send;
}

/// Read-only Change History, oldest entry first.
pub trait HistoryQuery: Send + Sync {
    fn entity_history(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
        page: PageRequest,
    ) -> impl Future<Output = Result<Page<HistoryEntry>, RepoError>> + Send;

    /// Every entry for a Project and the items in it, in the order recorded.
    fn project_history(
        &self,
        project_id: ProjectId,
    ) -> impl Future<Output = Result<Vec<HistoryEntry>, RepoError>> + Send;

    /// The entries for a Project and its items recorded in `[from, to)`, in
    /// the order recorded.
    fn project_history_between(
        &self,
        project_id: ProjectId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<HistoryEntry>, RepoError>> + Send;
}

/// Who an Action or Blocker must be assigned to, to be listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssigneeFilter {
    /// Assigned to no one.
    Unassigned,
    /// Assigned to this Person.
    Person(PersonId),
}

impl AssigneeFilter {
    /// Whether an item assigned to `assignee` passes this filter.
    #[must_use]
    pub fn matches(self, assignee: Option<PersonId>) -> bool {
        match self {
            Self::Unassigned => assignee.is_none(),
            Self::Person(id) => assignee == Some(id),
        }
    }
}

/// Which Actions to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionFilter {
    /// Only this Project's Actions; all Projects when `None`.
    pub project_id: Option<ProjectId>,
    pub state: ClosedFilter,
    /// Only Actions with this assignee.
    pub assignee: Option<AssigneeFilter>,
    /// Only Actions due on or before this date.
    pub due_by: Option<NaiveDate>,
}

/// The order Actions are listed in. Ties are broken by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionSort {
    /// Creation order (IDs are UUIDv7).
    #[default]
    Id,
    /// Project Code, then Reference number.
    Reference,
    /// Earliest due first; undated last.
    DueDate,
    /// Assignee display name; unassigned last.
    Assignee,
}

/// Read-only views of Actions.
pub trait ActionsQuery: Send + Sync {
    fn list_actions(
        &self,
        filter: ActionFilter,
        page: PageRequest,
        sort: ActionSort,
    ) -> impl Future<Output = Result<Page<ActionView>, RepoError>> + Send;

    fn get_action(
        &self,
        id: ActionId,
    ) -> impl Future<Output = Result<ActionView, RepoError>> + Send;
}

/// Which Blockers to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockerFilter {
    /// Only this Project's Blockers; all Projects when `None`.
    pub project_id: Option<ProjectId>,
    pub state: ClearedFilter,
    /// Only Blockers with this accountable Person.
    pub accountable: Option<AssigneeFilter>,
}

/// The order Blockers are listed in. Ties are broken by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockerSort {
    /// Creation order (IDs are UUIDv7).
    #[default]
    Id,
    /// Project Code, then Reference number.
    Reference,
    /// Accountable Person's display name; unassigned last.
    Accountable,
}

/// Read-only views of Blockers.
pub trait BlockersQuery: Send + Sync {
    fn list_blockers(
        &self,
        filter: BlockerFilter,
        page: PageRequest,
        sort: BlockerSort,
    ) -> impl Future<Output = Result<Page<BlockerView>, RepoError>> + Send;

    fn get_blocker(
        &self,
        id: BlockerId,
    ) -> impl Future<Output = Result<BlockerView, RepoError>> + Send;
}

/// Which Comments to list. Deleted Comments are always included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommentFilter {
    /// Only this Project's Comments, including those on its items.
    pub project_id: Option<ProjectId>,
    /// Only Comments about this target.
    pub target: Option<Target>,
}

/// Read-only views of Comments, oldest first (FR-126).
pub trait CommentsQuery: Send + Sync {
    fn list_comments(
        &self,
        filter: CommentFilter,
        page: PageRequest,
    ) -> impl Future<Output = Result<Page<CommentView>, RepoError>> + Send;

    fn get_comment(
        &self,
        id: CommentId,
    ) -> impl Future<Output = Result<CommentView, RepoError>> + Send;
}

/// Read-only views of Meetings.
pub trait MeetingsQuery: Send + Sync {
    /// A Project's Meetings, newest first, without their Minutes.
    fn list_meetings(
        &self,
        project_id: ProjectId,
        page: PageRequest,
    ) -> impl Future<Output = Result<Page<MeetingSummary>, RepoError>> + Send;

    fn get_meeting(
        &self,
        id: MeetingId,
    ) -> impl Future<Output = Result<MeetingView, RepoError>> + Send;
}
