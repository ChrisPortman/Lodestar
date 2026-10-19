//! The Acting Person session (FR-04, ADR-0006): a signed, `HttpOnly`,
//! `SameSite=Strict` cookie naming the Person. Nothing is stored server-side.

use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::request::Parts,
    routing::get,
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use lodestar_core::person::{PersonId, use_cases::get_person};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{AppState, Store, people::PersonBody, problem::Problem, request::JsonBody};

pub const COOKIE: &str = "lodestar_session";

/// About 400 days: the longest lifetime browsers honour.
const MAX_AGE_SECONDS: i64 = 400 * 24 * 60 * 60;

pub fn routes<S: Store>() -> Router<AppState<S>> {
    Router::new().route("/session", get(current::<S>).post(choose::<S>))
}

/// The Person named by a valid session cookie, if any. It is not checked
/// against the database; use cases do that where it matters.
pub struct MaybeActing(pub Option<PersonId>);

/// A valid session is required: 401 otherwise.
pub struct Acting(pub PersonId);

impl<S: Store> FromRequestParts<AppState<S>> for MaybeActing {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<S>,
    ) -> Result<Self, Self::Rejection> {
        let jar = SignedCookieJar::from_headers(&parts.headers, state.session_key.clone());
        let person = jar
            .get(COOKIE)
            .and_then(|c| Uuid::parse_str(c.value()).ok())
            .map(PersonId::from_uuid);
        Ok(Self(person))
    }
}

impl<S: Store> FromRequestParts<AppState<S>> for Acting {
    type Rejection = Problem;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<S>,
    ) -> Result<Self, Self::Rejection> {
        let MaybeActing(person) = MaybeActing::from_request_parts(parts, state)
            .await
            .unwrap_or_else(|never| match never {});
        person.map(Self).ok_or_else(Problem::unauthorized)
    }
}

/// A jar holding a fresh session cookie naming `person`.
pub fn cookie_for(key: &Key, person: PersonId) -> SignedCookieJar {
    let cookie = Cookie::build((COOKIE, person.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(cookie_max_age());
    SignedCookieJar::new(key.clone()).add(cookie)
}

fn cookie_max_age() -> cookie::time::Duration {
    cookie::time::Duration::seconds(MAX_AGE_SECONDS)
}

/// The current Acting Person.
#[derive(Debug, Serialize, ToSchema)]
pub struct Session {
    pub person: PersonBody,
}

/// Chooses the Acting Person.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewSession {
    pub person_id: Uuid,
}

/// Returns the current Acting Person.
#[utoipa::path(
    get,
    path = "/api/v1/session",
    tag = "session",
    operation_id = "get_session",
    responses(
        (status = 200, body = Session),
        (status = 401, description = "No valid session", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn current<S: Store>(
    State(state): State<AppState<S>>,
    Acting(person): Acting,
) -> Result<Json<Session>, Problem> {
    let view = get_person(&state.store, person)
        .await
        .map_err(|e| match e {
            lodestar_core::person::use_cases::PersonError::Repo(
                lodestar_core::ports::RepoError::NotFound,
            ) => Problem::unauthorized(),
            other => other.into(),
        })?;
    Ok(Json(Session {
        person: view.into(),
    }))
}

/// Sets the Acting Person, in a signed session cookie. Needs no session.
#[utoipa::path(
    post,
    path = "/api/v1/session",
    tag = "session",
    operation_id = "create_session",
    request_body = NewSession,
    responses(
        (status = 200, body = Session, headers(("set-cookie" = String, description = "The session cookie"))),
        (status = 422, description = "No such Person", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn choose<S: Store>(
    State(state): State<AppState<S>>,
    JsonBody(body): JsonBody<NewSession>,
) -> Result<(SignedCookieJar, Json<Session>), Problem> {
    let id = PersonId::from_uuid(body.person_id);
    let view = get_person(&state.store, id).await.map_err(|e| match e {
        lodestar_core::person::use_cases::PersonError::Repo(
            lodestar_core::ports::RepoError::NotFound,
        ) => Problem::invalid("person_id does not name a Person"),
        other => other.into(),
    })?;
    Ok((
        cookie_for(&state.session_key, id),
        Json(Session {
            person: view.into(),
        }),
    ))
}
