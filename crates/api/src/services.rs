//! The use cases the handlers call, behind an object-safe trait so handlers
//! are not generic over the store (which keeps utoipa's path docs simple).

use std::{future::Future, pin::Pin};

use lodestar_core::{
    person::{
        Person, PersonId, PersonView,
        use_cases::{self, PersonError},
    },
    ports::{Page, PageRequest, PeopleQuery, PeopleSort, ProjectSort, ProjectsQuery, UnitOfWork},
    project::{
        PersonRef, Project, ProjectCode, ProjectId, ProjectView, members,
        use_cases::{self as project_use_cases, NewProject, ProjectEdit, ProjectError},
    },
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

pub trait Projects: Send + Sync {
    fn list(
        &self,
        page: PageRequest,
        include_archived: bool,
        sort: ProjectSort,
    ) -> BoxFuture<'_, Result<Page<ProjectView>, ProjectError>>;

    fn get(&self, id: ProjectId) -> BoxFuture<'_, Result<ProjectView, ProjectError>>;

    fn suggest_code<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<ProjectCode>, ProjectError>>;

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewProject<'a>,
    ) -> BoxFuture<'a, Result<Project, ProjectError>>;

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        edit: ProjectEdit<'a>,
    ) -> BoxFuture<'a, Result<Project, ProjectError>>;

    fn members(&self, id: ProjectId) -> BoxFuture<'_, Result<Vec<PersonRef>, ProjectError>>;

    fn add_member(
        &self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        person: PersonId,
    ) -> BoxFuture<'_, Result<Project, ProjectError>>;

    fn remove_member(
        &self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        person: PersonId,
    ) -> BoxFuture<'_, Result<Project, ProjectError>>;

    fn set_archived(
        &self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        archived: bool,
    ) -> BoxFuture<'_, Result<Project, ProjectError>>;
}

impl<S: UnitOfWork + ProjectsQuery> Projects for S {
    fn list(
        &self,
        page: PageRequest,
        include_archived: bool,
        sort: ProjectSort,
    ) -> BoxFuture<'_, Result<Page<ProjectView>, ProjectError>> {
        Box::pin(project_use_cases::list_projects(
            self,
            page,
            include_archived,
            sort,
        ))
    }

    fn get(&self, id: ProjectId) -> BoxFuture<'_, Result<ProjectView, ProjectError>> {
        Box::pin(project_use_cases::get_project(self, id))
    }

    fn suggest_code<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<Option<ProjectCode>, ProjectError>> {
        Box::pin(project_use_cases::suggest_project_code(self, name))
    }

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewProject<'a>,
    ) -> BoxFuture<'a, Result<Project, ProjectError>> {
        Box::pin(project_use_cases::create_project(self, acting, input))
    }

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        edit: ProjectEdit<'a>,
    ) -> BoxFuture<'a, Result<Project, ProjectError>> {
        Box::pin(project_use_cases::update_project(
            self,
            acting,
            id,
            expected_version,
            edit,
        ))
    }

    fn members(&self, id: ProjectId) -> BoxFuture<'_, Result<Vec<PersonRef>, ProjectError>> {
        Box::pin(members::project_members(self, id))
    }

    fn add_member(
        &self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        person: PersonId,
    ) -> BoxFuture<'_, Result<Project, ProjectError>> {
        Box::pin(members::add_member(
            self,
            acting,
            id,
            expected_version,
            person,
        ))
    }

    fn remove_member(
        &self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        person: PersonId,
    ) -> BoxFuture<'_, Result<Project, ProjectError>> {
        Box::pin(members::remove_member(
            self,
            acting,
            id,
            expected_version,
            person,
        ))
    }

    fn set_archived(
        &self,
        acting: PersonId,
        id: ProjectId,
        expected_version: u32,
        archived: bool,
    ) -> BoxFuture<'_, Result<Project, ProjectError>> {
        Box::pin(project_use_cases::set_archived(
            self,
            acting,
            id,
            expected_version,
            archived,
        ))
    }
}
