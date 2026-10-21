mod support;

use axum::http::{Method, StatusCode};
use serde_json::json;
use support::{App, Reply, app};

/// Ada leads WEBR and has two browser sessions; Bob is a Member; Cy is not.
struct World {
    app: App,
    ada: String,
    ada_tab: String,
    ada_other_tab: String,
    bob_tab: String,
    cy_tab: String,
    lock: String,
}

async fn session_for(app: &App, person: &str) -> String {
    app.send(
        Method::POST,
        "/api/v1/session",
        &[],
        Some(json!({"person_id": person})),
    )
    .await
    .cookie()
}

async fn world() -> World {
    let app = app().await;
    let first = app.first_person("Ada").await;
    let ada_tab = first.cookie();
    let ada = first.body["id"].as_str().unwrap().to_owned();
    let mut ids = Vec::new();
    for name in ["Bob", "Cy"] {
        let reply = app
            .send(
                Method::POST,
                "/api/v1/people",
                &[("cookie", &ada_tab)],
                Some(json!({"display_name": name})),
            )
            .await;
        ids.push(reply.body["id"].as_str().unwrap().to_owned());
    }
    let project = app
        .send(
            Method::POST,
            "/api/v1/projects",
            &[("cookie", &ada_tab)],
            Some(json!({"name": "Web", "project_code": "WEBR"})),
        )
        .await;
    let project_id = project.body["id"].as_str().unwrap().to_owned();
    let added = app
        .send(
            Method::POST,
            &format!("/api/v1/projects/{project_id}/members"),
            &[("cookie", &ada_tab), ("if-match", project.header("etag"))],
            Some(json!({"person_id": ids[0]})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK);
    World {
        ada_other_tab: session_for(&app, &ada).await,
        bob_tab: session_for(&app, &ids[0]).await,
        cy_tab: session_for(&app, &ids[1]).await,
        app,
        ada,
        ada_tab,
        lock: format!("/api/v1/projects/{project_id}/meeting-lock"),
    }
}

impl World {
    async fn call(&self, method: Method, cookie: &str) -> Reply {
        self.app
            .send(method, &self.lock, &[("cookie", cookie)], None)
            .await
    }
}

#[tokio::test]
async fn the_leader_acquires_and_members_read_the_lock() {
    let w = world().await;
    let acquired = w.call(Method::POST, &w.ada_tab).await;
    assert_eq!(acquired.status, StatusCode::OK);
    assert_eq!(acquired.body["held_by_you"], true);

    let seen = w.call(Method::GET, &w.bob_tab).await;
    assert_eq!(seen.status, StatusCode::OK);
    assert_eq!(seen.body["leader"]["id"], w.ada.as_str());
    assert_eq!(seen.body["leader"]["display_name"], "Ada");
    assert_eq!(seen.body["acquired_at"], acquired.body["acquired_at"]);
    assert_eq!(seen.body["live"], true);
    assert_eq!(seen.body["held_by_you"], false);
    let keys: Vec<_> = seen.body.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["acquired_at", "held_by_you", "leader", "live"]);
}

#[tokio::test]
async fn acquiring_needs_a_session_and_the_project_leader() {
    let w = world().await;
    assert_eq!(
        w.call(Method::POST, "").await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        w.call(Method::POST, &w.bob_tab).await.status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn a_live_lock_held_by_another_session_is_a_conflict() {
    let w = world().await;
    w.call(Method::POST, &w.ada_tab).await;
    assert_eq!(
        w.call(Method::POST, &w.ada_other_tab).await.status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        w.call(Method::POST, &w.ada_tab).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn only_the_holding_session_heartbeats() {
    let w = world().await;
    assert_eq!(
        w.call(Method::PUT, &w.ada_tab).await.status,
        StatusCode::CONFLICT
    );
    w.call(Method::POST, &w.ada_tab).await;
    let beat = w.call(Method::PUT, &w.ada_tab).await;
    assert_eq!(beat.status, StatusCode::OK);
    assert_eq!(beat.body["live"], true);
    assert_eq!(
        w.call(Method::PUT, &w.ada_other_tab).await.status,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn only_the_holding_session_cancels_and_the_lock_is_gone() {
    let w = world().await;
    w.call(Method::POST, &w.ada_tab).await;
    assert_eq!(
        w.call(Method::DELETE, &w.ada_other_tab).await.status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        w.call(Method::DELETE, &w.ada_tab).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        w.call(Method::GET, &w.bob_tab).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn reading_needs_a_member_and_a_lock() {
    let w = world().await;
    assert_eq!(
        w.call(Method::GET, &w.bob_tab).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        w.call(Method::GET, &w.cy_tab).await.status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn an_unknown_project_is_not_found() {
    let w = world().await;
    let reply = w
        .app
        .send(
            Method::POST,
            "/api/v1/projects/0192f0c4-0000-7000-8000-000000000000/meeting-lock",
            &[("cookie", &w.ada_tab)],
            None,
        )
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}
