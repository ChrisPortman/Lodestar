//! Unit of work, repository and query ports (ADR-0007).
//!
//! Adapters live in `lodestar-datarepo`; in-memory fakes live in
//! [`crate::test_support`]. No adapter types appear here.

use std::future::Future;

use uuid::Uuid;

use crate::history::{ChangeContext, EntityType, HistoryEntry, HistoryEntryId};
use crate::person::{Person, PersonId, PersonView};

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

    fn references(&mut self) -> impl ReferenceCounter + Send + '_;

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

/// Read-only Change History, oldest entry first.
pub trait HistoryQuery: Send + Sync {
    fn entity_history(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
        page: PageRequest,
    ) -> impl Future<Output = Result<Page<HistoryEntry>, RepoError>> + Send;
}
