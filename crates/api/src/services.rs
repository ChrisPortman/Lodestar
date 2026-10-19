//! The use cases the handlers call, behind an object-safe trait so handlers
//! are not generic over the store (which keeps utoipa's path docs simple).

use std::{future::Future, pin::Pin};

use lodestar_core::{
    person::{
        Person, PersonId, PersonView,
        use_cases::{self, PersonError},
    },
    ports::{Page, PageRequest, PeopleQuery, PeopleSort, ProjectSort, ProjectsQuery, UnitOfWork},
    ports::{RiskFilter, RiskSort, RisksQuery},
    project::{
        PersonRef, Project, ProjectCode, ProjectId, ProjectView, members,
        use_cases::{self as project_use_cases, NewProject, ProjectEdit, ProjectError},
    },
    risk::{
        Risk, RiskEdit, RiskId, RiskView,
        use_cases::{self as risk_use_cases, NewRisk, RiskError},
    },
};

use lodestar_core::action::{
    Action, ActionId, ActionView,
    use_cases::{self as action_use_cases, ActionEdit, ActionError, ActionFilter, NewAction},
};
use lodestar_core::ports::ActionsQuery;
use lodestar_core::{
    action::ActionId as BlockedActionId,
    blocker::{
        Blocker, BlockerId, BlockerView,
        use_cases::{
            self as blocker_use_cases, BlockerEdit, BlockerError, BlockerFilter, NewBlocker,
        },
    },
    ports::BlockersQuery,
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

pub trait Risks: Send + Sync {
    fn list(
        &self,
        page: PageRequest,
        filter: RiskFilter,
        sort: RiskSort,
    ) -> BoxFuture<'_, Result<Page<RiskView>, RiskError>>;

    fn get(&self, id: RiskId) -> BoxFuture<'_, Result<RiskView, RiskError>>;

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewRisk<'a>,
    ) -> BoxFuture<'a, Result<Risk, RiskError>>;

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: RiskId,
        expected_version: u32,
        edit: RiskEdit<'a>,
    ) -> BoxFuture<'a, Result<Risk, RiskError>>;

    fn set_closed(
        &self,
        acting: PersonId,
        id: RiskId,
        expected_version: u32,
        closed: bool,
    ) -> BoxFuture<'_, Result<Risk, RiskError>>;
}

impl<S: UnitOfWork + RisksQuery> Risks for S {
    fn list(
        &self,
        page: PageRequest,
        filter: RiskFilter,
        sort: RiskSort,
    ) -> BoxFuture<'_, Result<Page<RiskView>, RiskError>> {
        Box::pin(risk_use_cases::list_risks(self, page, filter, sort))
    }

    fn get(&self, id: RiskId) -> BoxFuture<'_, Result<RiskView, RiskError>> {
        Box::pin(risk_use_cases::get_risk(self, id))
    }

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewRisk<'a>,
    ) -> BoxFuture<'a, Result<Risk, RiskError>> {
        Box::pin(risk_use_cases::create_risk(self, acting, input))
    }

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: RiskId,
        expected_version: u32,
        edit: RiskEdit<'a>,
    ) -> BoxFuture<'a, Result<Risk, RiskError>> {
        Box::pin(risk_use_cases::update_risk(
            self,
            acting,
            id,
            expected_version,
            edit,
        ))
    }

    fn set_closed(
        &self,
        acting: PersonId,
        id: RiskId,
        expected_version: u32,
        closed: bool,
    ) -> BoxFuture<'_, Result<Risk, RiskError>> {
        Box::pin(risk_use_cases::set_closed(
            self,
            acting,
            id,
            expected_version,
            closed,
        ))
    }
}

pub trait Actions: Send + Sync {
    fn list(
        &self,
        filter: ActionFilter,
        page: PageRequest,
    ) -> BoxFuture<'_, Result<Page<ActionView>, ActionError>>;

    fn get(&self, id: ActionId) -> BoxFuture<'_, Result<ActionView, ActionError>>;

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewAction<'a>,
    ) -> BoxFuture<'a, Result<Action, ActionError>>;

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: ActionId,
        expected_version: u32,
        edit: ActionEdit<'a>,
    ) -> BoxFuture<'a, Result<Action, ActionError>>;

    fn set_closed(
        &self,
        acting: PersonId,
        id: ActionId,
        expected_version: u32,
        closed: bool,
    ) -> BoxFuture<'_, Result<Action, ActionError>>;
}

impl<S: UnitOfWork + ActionsQuery> Actions for S {
    fn list(
        &self,
        filter: ActionFilter,
        page: PageRequest,
    ) -> BoxFuture<'_, Result<Page<ActionView>, ActionError>> {
        Box::pin(action_use_cases::list_actions(self, filter, page))
    }

    fn get(&self, id: ActionId) -> BoxFuture<'_, Result<ActionView, ActionError>> {
        Box::pin(action_use_cases::get_action(self, id))
    }

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewAction<'a>,
    ) -> BoxFuture<'a, Result<Action, ActionError>> {
        Box::pin(action_use_cases::create_action(self, acting, input))
    }

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: ActionId,
        expected_version: u32,
        edit: ActionEdit<'a>,
    ) -> BoxFuture<'a, Result<Action, ActionError>> {
        Box::pin(action_use_cases::update_action(
            self,
            acting,
            id,
            expected_version,
            edit,
        ))
    }

    fn set_closed(
        &self,
        acting: PersonId,
        id: ActionId,
        expected_version: u32,
        closed: bool,
    ) -> BoxFuture<'_, Result<Action, ActionError>> {
        Box::pin(action_use_cases::set_action_closed(
            self,
            acting,
            id,
            expected_version,
            closed,
        ))
    }
}

pub trait Blockers: Send + Sync {
    fn list(
        &self,
        filter: BlockerFilter,
        page: PageRequest,
    ) -> BoxFuture<'_, Result<Page<BlockerView>, BlockerError>>;

    fn get(&self, id: BlockerId) -> BoxFuture<'_, Result<BlockerView, BlockerError>>;

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewBlocker<'a>,
    ) -> BoxFuture<'a, Result<Blocker, BlockerError>>;

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        edit: BlockerEdit<'a>,
    ) -> BoxFuture<'a, Result<Blocker, BlockerError>>;

    fn set_cleared(
        &self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        cleared: bool,
    ) -> BoxFuture<'_, Result<Blocker, BlockerError>>;

    fn link(
        &self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        action: BlockedActionId,
    ) -> BoxFuture<'_, Result<Blocker, BlockerError>>;

    fn unlink(
        &self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        action: BlockedActionId,
    ) -> BoxFuture<'_, Result<Blocker, BlockerError>>;
}

impl<S: UnitOfWork + BlockersQuery> Blockers for S {
    fn list(
        &self,
        filter: BlockerFilter,
        page: PageRequest,
    ) -> BoxFuture<'_, Result<Page<BlockerView>, BlockerError>> {
        Box::pin(blocker_use_cases::list_blockers(self, filter, page))
    }

    fn get(&self, id: BlockerId) -> BoxFuture<'_, Result<BlockerView, BlockerError>> {
        Box::pin(blocker_use_cases::get_blocker(self, id))
    }

    fn create<'a>(
        &'a self,
        acting: PersonId,
        input: NewBlocker<'a>,
    ) -> BoxFuture<'a, Result<Blocker, BlockerError>> {
        Box::pin(blocker_use_cases::create_blocker(self, acting, input))
    }

    fn update<'a>(
        &'a self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        edit: BlockerEdit<'a>,
    ) -> BoxFuture<'a, Result<Blocker, BlockerError>> {
        Box::pin(blocker_use_cases::update_blocker(
            self,
            acting,
            id,
            expected_version,
            edit,
        ))
    }

    fn set_cleared(
        &self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        cleared: bool,
    ) -> BoxFuture<'_, Result<Blocker, BlockerError>> {
        Box::pin(blocker_use_cases::set_blocker_cleared(
            self,
            acting,
            id,
            expected_version,
            cleared,
        ))
    }

    fn link(
        &self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        action: BlockedActionId,
    ) -> BoxFuture<'_, Result<Blocker, BlockerError>> {
        Box::pin(blocker_use_cases::link_blocked_action(
            self,
            acting,
            id,
            expected_version,
            action,
        ))
    }

    fn unlink(
        &self,
        acting: PersonId,
        id: BlockerId,
        expected_version: u32,
        action: BlockedActionId,
    ) -> BoxFuture<'_, Result<Blocker, BlockerError>> {
        Box::pin(blocker_use_cases::unlink_blocked_action(
            self,
            acting,
            id,
            expected_version,
            action,
        ))
    }
}
