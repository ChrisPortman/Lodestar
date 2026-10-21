mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

/// Ada leads WEBR, with Bob a Member. Before the meeting there is one Action
/// and one Blocker; then Ada's `ada_tab` acquires the Meeting Lock. A stale
/// lock is covered in core, where the clock can be moved.
struct World {
    app: App,
    ada_tab: String,
    ada_other_tab: String,
    bob_tab: String,
    bob: String,
    project: String,
    action: String,
    blocker: String,
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
    let bob = app
        .send(
            Method::POST,
            "/api/v1/people",
            &[("cookie", &ada_tab)],
            Some(json!({"display_name": "Bob"})),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
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
            Some(json!({"person_id": bob})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK);
    let id_of = |reply: Reply| reply.body["id"].as_str().unwrap().to_owned();
    let action = id_of(
        app.send(
            Method::POST,
            "/api/v1/actions",
            &[("cookie", &ada_tab)],
            Some(json!({"project_id": project_id, "description": "Book"})),
        )
        .await,
    );
    let blocker = id_of(
        app.send(
            Method::POST,
            "/api/v1/blockers",
            &[("cookie", &ada_tab)],
            Some(json!({"project_id": project_id, "description": "No room"})),
        )
        .await,
    );
    let locked = app
        .send(
            Method::POST,
            &format!("/api/v1/projects/{project_id}/meeting-lock"),
            &[("cookie", &ada_tab)],
            None,
        )
        .await;
    assert_eq!(locked.status, StatusCode::OK);
    World {
        ada_other_tab: session_for(&app, &ada).await,
        bob_tab: session_for(&app, &bob).await,
        app,
        ada_tab,
        bob,
        project: project_id,
        action,
        blocker,
    }
}

impl World {
    async fn send(&self, method: Method, path: &str, cookie: &str, body: Option<Value>) -> Reply {
        let etag = self.app.get(path, cookie).await;
        let etag = etag
            .headers
            .get("etag")
            .map(|v| v.to_str().unwrap().to_owned());
        let mut headers = vec![("cookie", cookie)];
        if let Some(etag) = etag.as_deref() {
            headers.push(("if-match", etag));
        }
        self.app.send(method, path, &headers, body).await
    }

    async fn etag(&self, path: &str) -> String {
        self.app
            .get(path, &self.ada_tab)
            .await
            .header("etag")
            .to_owned()
    }
}

/// A 409 problem naming the meeting's leader and when it started.
fn assert_locked(reply: &Reply) {
    assert_eq!(reply.status, StatusCode::CONFLICT, "{:?}", reply.body);
    assert_eq!(reply.header("content-type"), "application/problem+json");
    assert_eq!(reply.body["meeting"]["leader"]["display_name"], "Ada");
    assert!(reply.body["meeting"]["since"].is_string());
    let detail = reply.body["detail"].as_str().unwrap();
    assert!(detail.contains("Ada"), "{detail}");
}

#[tokio::test]
async fn actions_are_refused_to_other_sessions() {
    let w = world().await;
    let create = json!({"project_id": w.project, "description": "Late"});
    assert_locked(
        &w.send(
            Method::POST,
            "/api/v1/actions",
            &w.bob_tab,
            Some(create.clone()),
        )
        .await,
    );
    assert_locked(
        &w.send(
            Method::POST,
            "/api/v1/actions",
            &w.ada_other_tab,
            Some(create.clone()),
        )
        .await,
    );
    let path = format!("/api/v1/actions/{}", w.action);
    assert_locked(
        &w.send(
            Method::PUT,
            &path,
            &w.bob_tab,
            Some(json!({"description": "Edited", "assignee": null, "due_date": null})),
        )
        .await,
    );
    assert_locked(
        &w.send(
            Method::PATCH,
            &path,
            &w.bob_tab,
            Some(json!({"closed": true})),
        )
        .await,
    );

    let held = w
        .send(Method::POST, "/api/v1/actions", &w.ada_tab, Some(create))
        .await;
    assert_eq!(held.status, StatusCode::CREATED);
}

#[tokio::test]
async fn risks_are_refused_to_other_sessions() {
    let w = world().await;
    let reply = w
        .send(
            Method::POST,
            "/api/v1/risks",
            &w.bob_tab,
            Some(json!({"project_id": w.project, "description": "Rain", "likelihood": 2, "impact": 2})),
        )
        .await;
    assert_locked(&reply);
}

#[tokio::test]
async fn blockers_and_their_links_are_refused_to_other_sessions() {
    let w = world().await;
    assert_locked(
        &w.send(
            Method::POST,
            "/api/v1/blockers",
            &w.bob_tab,
            Some(json!({"project_id": w.project, "description": "Late"})),
        )
        .await,
    );
    let blocker = format!("/api/v1/blockers/{}", w.blocker);
    let link = w
        .app
        .send(
            Method::POST,
            &format!("{blocker}/blocked_actions"),
            &[
                ("cookie", &w.bob_tab),
                ("if-match", &w.etag(&blocker).await),
            ],
            Some(json!({"action_id": w.action})),
        )
        .await;
    assert_locked(&link);
}

#[tokio::test]
async fn comments_are_refused_to_other_sessions_even_on_the_project() {
    let w = world().await;
    let reply = w
        .send(
            Method::POST,
            "/api/v1/comments",
            &w.bob_tab,
            Some(json!({"project_id": w.project, "target_type": "project", "target_id": w.project, "body": "Hi"})),
        )
        .await;
    assert_locked(&reply);
}

#[tokio::test]
async fn the_project_itself_is_refused_to_other_sessions() {
    let w = world().await;
    let reply = w
        .send(
            Method::PUT,
            &format!("/api/v1/projects/{}", w.project),
            &w.bob_tab,
            Some(json!({"name": "Renamed", "description": null, "project_leader": null, "health_rating": {"rating": "red"}})),
        )
        .await;
    assert_locked(&reply);
}

#[tokio::test]
async fn archiving_is_refused_even_to_the_holding_session() {
    let w = world().await;
    let reply = w
        .send(
            Method::PATCH,
            &format!("/api/v1/projects/{}", w.project),
            &w.ada_tab,
            Some(json!({"archived": true})),
        )
        .await;
    assert_locked(&reply);
}

#[tokio::test]
async fn members_and_people_stay_open() {
    let w = world().await;
    let members = format!("/api/v1/projects/{}", w.project);
    let removed = w
        .app
        .send(
            Method::DELETE,
            &format!("{members}/members/{}", w.bob),
            &[
                ("cookie", &w.ada_other_tab),
                ("if-match", &w.etag(&members).await),
            ],
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.body);
    let added = w
        .app
        .send(
            Method::POST,
            &format!("{members}/members"),
            &[
                ("cookie", &w.ada_other_tab),
                ("if-match", &w.etag(&members).await),
            ],
            Some(json!({"person_id": w.bob})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.body);

    let person = w
        .app
        .send(
            Method::POST,
            "/api/v1/people",
            &[("cookie", &w.bob_tab)],
            Some(json!({"display_name": "Cy"})),
        )
        .await;
    assert_eq!(person.status, StatusCode::CREATED);
}
