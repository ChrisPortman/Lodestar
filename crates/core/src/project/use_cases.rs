//! Project use cases. Each change runs in one unit of work.

use crate::history::ChangeContext;
use crate::person::PersonId;
use crate::ports::{
    Page, PageRequest, PersonRepository, ProjectRepository, ProjectsQuery, RepoError, Transaction,
    UnitOfWork,
};
use crate::project::guard::{GuardError, writable_project};
use crate::project::{
    Description, DescriptionError, LeaderNotAMember, Project, ProjectCode, ProjectCodeError,
    ProjectId, ProjectName, ProjectNameError, ProjectView, suggest_code,
};

pub use crate::ports::ProjectSort;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectError {
    #[error(transparent)]
    InvalidName(#[from] ProjectNameError),
    #[error(transparent)]
    InvalidDescription(#[from] DescriptionError),
    #[error(transparent)]
    InvalidCode(#[from] ProjectCodeError),
    #[error("the project code is already in use")]
    CodeTaken,
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error("the project leader must be a member")]
    LeaderNotAMember,
    #[error("that person is not a member of the project")]
    NotAMember,
    #[error("that person does not exist")]
    UnknownPerson,
    #[error("the acting person does not exist")]
    UnknownActingPerson,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl From<LeaderNotAMember> for ProjectError {
    fn from(_: LeaderNotAMember) -> Self {
        Self::LeaderNotAMember
    }
}

impl From<GuardError> for ProjectError {
    fn from(error: GuardError) -> Self {
        match error {
            GuardError::Archived => Self::Archived,
            GuardError::Repo(e) => Self::Repo(e),
        }
    }
}

/// What a new Project is created from.
#[derive(Debug, Clone, Copy)]
pub struct NewProject<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub project_code: &'a str,
}

/// Creates a Project (FR-10, FR-11). The Acting Person becomes a Member and
/// the Project Leader (FR-12).
pub async fn create_project<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    input: NewProject<'_>,
) -> Result<Project, ProjectError> {
    let name = ProjectName::new(input.name)?;
    let description = Description::optional(input.description)?;
    let code = ProjectCode::new(input.project_code)?;
    let mut tx = uow.begin().await?;
    match tx.people().load(acting).await {
        Err(RepoError::NotFound) => return Err(ProjectError::UnknownActingPerson),
        other => other?,
    };
    let project = Project::create(name, description, code, acting);
    tx.projects()
        .save(None, &project, &ChangeContext::by(acting))
        .await
        .map_err(|e| match e {
            RepoError::UniqueViolation { .. } => ProjectError::CodeTaken,
            e => e.into(),
        })?;
    tx.commit().await?;
    Ok(project)
}

/// Renames and re-describes a Project and sets its Project Leader, who must
/// be a Member (FR-13). Its code never changes (FR-11).
pub async fn update_project<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: ProjectId,
    expected_version: u32,
    name: &str,
    description: Option<&str>,
    project_leader: Option<PersonId>,
) -> Result<Project, ProjectError> {
    let name = ProjectName::new(name)?;
    let description = Description::optional(description)?;
    let mut tx = uow.begin().await?;
    let before = writable_project(&mut tx, id).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    let after = before.edited(name, description, project_leader)?;
    tx.projects()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Archives or unarchives a Project (FR-14). Setting the state it already has
/// changes nothing.
pub async fn set_archived<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: ProjectId,
    expected_version: u32,
    archived: bool,
) -> Result<Project, ProjectError> {
    let mut tx = uow.begin().await?;
    let before = tx.projects().load(id).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if before.archived() == archived {
        return Ok(before);
    }
    let after = before.with_archived(archived);
    tx.projects()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn list_projects<Q: ProjectsQuery>(
    query: &Q,
    page: PageRequest,
    include_archived: bool,
    sort: ProjectSort,
) -> Result<Page<ProjectView>, ProjectError> {
    Ok(query.list_projects(page, include_archived, sort).await?)
}

pub async fn get_project<Q: ProjectsQuery>(
    query: &Q,
    id: ProjectId,
) -> Result<ProjectView, ProjectError> {
    Ok(query.get_project(id).await?)
}

/// Suggests a code for a Project called `name` that no Project uses yet (FR-16).
pub async fn suggest_project_code<Q: ProjectsQuery>(
    query: &Q,
    name: &str,
) -> Result<Option<ProjectCode>, ProjectError> {
    let Some(plain) = suggest_code(name, |_| false) else {
        return Ok(None);
    };
    let taken = query.project_codes_starting_with(plain.as_str()).await?;
    Ok(suggest_code(name, |code| taken.contains(code)))
}
