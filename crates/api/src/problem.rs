//! RFC 9457 problem details, and the mapping from domain and port errors onto
//! them (NFR-13).

use axum::{
    Json,
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use lodestar_core::{
    action::use_cases::ActionError, blocker::use_cases::BlockerError,
    comment::use_cases::CommentError, meeting_lock::use_cases::LockError,
    person::use_cases::PersonError, ports::RepoError, project::use_cases::ProjectError,
    risk::use_cases::RiskError,
};
use serde::Serialize;
use utoipa::ToSchema;

/// An RFC 9457 problem details object, served as `application/problem+json`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Problem {
    /// Always `about:blank`: the status code says what went wrong.
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip)]
    allow: Option<&'static str>,
}

impl Problem {
    pub fn new(status: StatusCode) -> Self {
        Self {
            kind: "about:blank",
            title: status.canonical_reason().unwrap_or("Error").to_owned(),
            status: status.as_u16(),
            detail: None,
            allow: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND)
    }

    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY).detail(detail)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).detail("no valid session; choose an Acting Person")
    }

    pub fn stale() -> Self {
        Self::new(StatusCode::PRECONDITION_FAILED)
            .detail("the item has changed since it was read; reload it")
    }

    /// 405, with the `Allow` header listing the methods that are supported.
    pub fn method_not_allowed(allow: &'static str) -> Self {
        let mut problem =
            Self::new(StatusCode::METHOD_NOT_ALLOWED).detail("nothing is deleted (ADR-0006)");
        problem.allow = Some(allow);
        problem
    }

    fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = self.status();
        let allow = self.allow;
        let mut response = (status, Json(self)).into_response();
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        if let Some(allow) = allow {
            headers.insert(header::ALLOW, HeaderValue::from_static(allow));
        }
        response
    }
}

impl From<RepoError> for Problem {
    fn from(error: RepoError) -> Self {
        match error {
            RepoError::NotFound => Self::not_found(),
            RepoError::Conflict => Self::stale(),
            RepoError::UniqueViolation { .. } => {
                Self::new(StatusCode::CONFLICT).detail(error.to_string())
            }
            RepoError::Unexpected(message) => {
                tracing::error!(%message, "unexpected storage error");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

impl From<PersonError> for Problem {
    fn from(error: PersonError) -> Self {
        match error {
            PersonError::InvalidDisplayName(e) => Self::invalid(e.to_string()),
            PersonError::Repo(e) => e.into(),
        }
    }
}

impl From<ProjectError> for Problem {
    fn from(error: ProjectError) -> Self {
        match error {
            ProjectError::InvalidName(_)
            | ProjectError::InvalidDescription(_)
            | ProjectError::InvalidCode(_)
            | ProjectError::LeaderNotAMember
            | ProjectError::UnknownPerson => Self::invalid(error.to_string()),
            ProjectError::NotAMember => Self::not_found().detail(error.to_string()),
            ProjectError::CodeTaken | ProjectError::Archived => {
                Self::new(StatusCode::CONFLICT).detail(error.to_string())
            }
            ProjectError::UnknownActingPerson => Self::unauthorized(),
            ProjectError::Repo(e) => e.into(),
        }
    }
}

impl From<RiskError> for Problem {
    fn from(error: RiskError) -> Self {
        match error {
            RiskError::InvalidDescription(_)
            | RiskError::InvalidRating(_)
            | RiskError::UnknownProject => Self::invalid(error.to_string()),
            RiskError::Archived => Self::new(StatusCode::CONFLICT).detail(error.to_string()),
            RiskError::Repo(e) => e.into(),
        }
    }
}

impl From<JsonRejection> for Problem {
    fn from(rejection: JsonRejection) -> Self {
        let status = match rejection {
            JsonRejection::JsonDataError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            _ => rejection.status(),
        };
        Self::new(status).detail(rejection.body_text())
    }
}

impl From<PathRejection> for Problem {
    fn from(rejection: PathRejection) -> Self {
        Self::new(StatusCode::BAD_REQUEST).detail(rejection.body_text())
    }
}

impl From<QueryRejection> for Problem {
    fn from(rejection: QueryRejection) -> Self {
        Self::invalid(rejection.body_text())
    }
}

impl From<BlockerError> for Problem {
    fn from(error: BlockerError) -> Self {
        match error {
            BlockerError::InvalidDescription(_)
            | BlockerError::AccountableNotAMember
            | BlockerError::UnknownProject
            | BlockerError::UnknownAction
            | BlockerError::ActionInAnotherProject => Self::invalid(error.to_string()),
            BlockerError::Archived => Self::new(StatusCode::CONFLICT).detail(error.to_string()),
            BlockerError::Repo(e) => e.into(),
        }
    }
}

impl From<ActionError> for Problem {
    fn from(error: ActionError) -> Self {
        match error {
            ActionError::InvalidDescription(_)
            | ActionError::AssigneeNotAMember
            | ActionError::UnknownProject => Self::invalid(error.to_string()),
            ActionError::Archived => Self::new(StatusCode::CONFLICT).detail(error.to_string()),
            ActionError::Repo(e) => e.into(),
        }
    }
}

impl From<CommentError> for Problem {
    fn from(error: CommentError) -> Self {
        match error {
            CommentError::InvalidBody(_)
            | CommentError::UnknownProject
            | CommentError::UnknownTarget => Self::invalid(error.to_string()),
            CommentError::NotAMember | CommentError::NotTheAuthor => {
                Self::new(StatusCode::FORBIDDEN).detail(error.to_string())
            }
            CommentError::Archived | CommentError::Deleted => {
                Self::new(StatusCode::CONFLICT).detail(error.to_string())
            }
            CommentError::Repo(e) => e.into(),
        }
    }
}

impl From<LockError> for Problem {
    fn from(error: LockError) -> Self {
        match error {
            LockError::UnknownProject | LockError::NoLock => {
                Self::not_found().detail(error.to_string())
            }
            LockError::NotTheLeader | LockError::NotAMember => {
                Self::new(StatusCode::FORBIDDEN).detail(error.to_string())
            }
            // A Conflict here is a race with another session's change to the lock.
            LockError::Held | LockError::NotHeld | LockError::Repo(RepoError::Conflict) => {
                Self::new(StatusCode::CONFLICT).detail(error.to_string())
            }
            LockError::Repo(e) => e.into(),
        }
    }
}
