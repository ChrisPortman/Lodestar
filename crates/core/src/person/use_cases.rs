//! Person use cases. Each change runs in one unit of work.

use crate::history::ChangeContext;
use crate::person::{DisplayNameError, Person, PersonId, PersonView};
use crate::ports::{
    Page, PageRequest, PeopleQuery, PersonRepository, RepoError, Transaction, UnitOfWork,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PersonError {
    #[error(transparent)]
    InvalidDisplayName(#[from] DisplayNameError),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// Creates a Person (FR-01). With no Acting Person, as when someone first
/// adds themselves, the new Person is recorded as having created themselves.
pub async fn create_person<U: UnitOfWork>(
    uow: &U,
    acting: Option<PersonId>,
    display_name: &str,
) -> Result<Person, PersonError> {
    let person = Person::create(display_name)?;
    let ctx = ChangeContext::by(acting.unwrap_or(person.id()));
    let mut tx = uow.begin().await?;
    tx.people().save(None, &person, &ctx).await?;
    tx.commit().await?;
    Ok(person)
}

/// Changes a Person's display name (FR-05).
pub async fn rename_person<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: PersonId,
    display_name: &str,
) -> Result<Person, PersonError> {
    let mut tx = uow.begin().await?;
    let before = tx.people().load(id).await?;
    let after = before.renamed(display_name)?;
    tx.people()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn list_people<Q: PeopleQuery>(
    query: &Q,
    page: PageRequest,
) -> Result<Page<PersonView>, PersonError> {
    Ok(query.list_people(page).await?)
}

pub async fn get_person<Q: PeopleQuery>(
    query: &Q,
    id: PersonId,
) -> Result<PersonView, PersonError> {
    Ok(query.get_person(id).await?)
}
