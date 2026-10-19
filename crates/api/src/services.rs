//! The use cases the handlers call, behind an object-safe trait so handlers
//! are not generic over the store (which keeps utoipa's path docs simple).

use std::{future::Future, pin::Pin};

use lodestar_core::{
    person::{
        Person, PersonId, PersonView,
        use_cases::{self, PersonError},
    },
    ports::{Page, PageRequest, PeopleQuery, PeopleSort, UnitOfWork},
};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait People: Send + Sync {
    fn list(
        &self,
        page: PageRequest,
        sort: PeopleSort,
    ) -> BoxFuture<'_, Result<Page<PersonView>, PersonError>>;

    fn get(&self, id: PersonId) -> BoxFuture<'_, Result<PersonView, PersonError>>;

    fn create<'a>(
        &'a self,
        acting: Option<PersonId>,
        display_name: &'a str,
    ) -> BoxFuture<'a, Result<Person, PersonError>>;

    fn rename<'a>(
        &'a self,
        acting: PersonId,
        id: PersonId,
        expected_version: u32,
        display_name: &'a str,
    ) -> BoxFuture<'a, Result<Person, PersonError>>;
}

impl<S: UnitOfWork + PeopleQuery> People for S {
    fn list(
        &self,
        page: PageRequest,
        sort: PeopleSort,
    ) -> BoxFuture<'_, Result<Page<PersonView>, PersonError>> {
        Box::pin(use_cases::list_people(self, page, sort))
    }

    fn get(&self, id: PersonId) -> BoxFuture<'_, Result<PersonView, PersonError>> {
        Box::pin(use_cases::get_person(self, id))
    }

    fn create<'a>(
        &'a self,
        acting: Option<PersonId>,
        display_name: &'a str,
    ) -> BoxFuture<'a, Result<Person, PersonError>> {
        Box::pin(use_cases::create_person(self, acting, display_name))
    }

    fn rename<'a>(
        &'a self,
        acting: PersonId,
        id: PersonId,
        expected_version: u32,
        display_name: &'a str,
    ) -> BoxFuture<'a, Result<Person, PersonError>> {
        Box::pin(use_cases::rename_person(
            self,
            acting,
            id,
            expected_version,
            display_name,
        ))
    }
}
