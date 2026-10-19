mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

/// Ada's session and Project, with Bob added as a Member.
struct World {
    app: App,
    cookie: String,
    project: String,
    project_etag: String,
    bob: String,
}

async fn world() -> World {
    let app = app().await;
    let cookie = app.first_person("Ada").await.cookie();
    let bob = app
        .send(
            Method::POST,
            "/api/v1/people",
            &[("cookie", &cookie)],
            Some(json!({"display_name": "Bob"})),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let project = w_project(&app, &cookie, "WEBR").await;
    let added = app
        .send(
            Method::POST,
            &format!("/api/v1/projects/{project}/members"),
            &[("cookie", &cookie), ("if-match", "\"1\"")],
            Some(json!({"person_id": bob})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK);
    World {
        app,
        cookie,
        project,
        project_etag: added.header("etag").to_owned(),
        bob,
    }
}

async fn w_project(app: &App, cookie: &str, code: &str) -> String {
    app.send(
        Method::POST,
        "/api/v1/projects",
        &[("cookie", cookie)],
        Some(json!({"name": code, "project_code": code})),
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

impl World {
    async fn create(&self, body: Value) -> Reply {
        self.app
            .send(
                Method::POST,
                "/api/v1/blockers",
                &[("cookie", &self.cookie)],
                Some(body),
            )
            .await
    }

    async fn new_blocker(&self, description: &str, accountable: Option<&str>) -> Reply {
        let reply = self
            .create(json!({
                "project_id": self.project,
                "description": description,
                "accountable": accountable.map(|id| json!({"id": id})),
            }))
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
        reply
    }

    async fn new_action_in(&self, project: &str) -> String {
        let reply = self
            .app
            .send(
                Method::POST,
                "/api/v1/actions",
                &[("cookie", &self.cookie)],
                Some(json!({"project_id": project, "description": "Do it"})),
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn get(&self, path: &str) -> Reply {
        self.app.send(Method::GET, path, &[], None).await
    }

    async fn patch(&self, id: &str, etag: &str, cleared: bool) -> Reply {
        self.app
            .send(
                Method::PATCH,
                &format!("/api/v1/blockers/{id}"),
                &[
                    ("cookie", &self.cookie),
                    ("if-match", etag),
                    ("content-type", "application/merge-patch+json"),
                ],
                Some(json!({"cleared": cleared})),
            )
            .await
    }

    async fn link(&self, id: &str, etag: &str, action: &str) -> Reply {
        self.app
            .send(
                Method::POST,
                &format!("/api/v1/blockers/{id}/blocked_actions"),
                &[("cookie", &self.cookie), ("if-match", etag)],
                Some(json!({"action_id": action})),
            )
            .await
    }

    async fn unlink(&self, id: &str, etag: &str, action: &str) -> Reply {
        self.app
            .send(
                Method::DELETE,
                &format!("/api/v1/blockers/{id}/blocked_actions/{action}"),
                &[("cookie", &self.cookie), ("if-match", etag)],
                None,
            )
            .await
    }
}

#[tokio::test]
async fn creating_a_blocker_gives_it_a_reference_and_location() {
    let w = world().await;
    w.new_blocker("First", None).await;
    let reply = w.new_blocker("No test env", Some(&w.bob)).await;
    let id = reply.body["id"].as_str().unwrap();
    assert_eq!(reply.header("location"), format!("/api/v1/blockers/{id}"));
    assert_eq!(reply.header("etag"), "\"1\"");
    assert_eq!(reply.body["reference"], "B-002");
    assert_eq!(reply.body["project_code"], "WEBR");
    assert_eq!(reply.body["accountable"]["display_name"], "Bob");
    assert_eq!(reply.body["cleared"], false);
    assert_eq!(reply.body["blocked_actions"], json!([]));

    let fetched = w.get(&format!("/api/v1/blockers/{id}")).await;
    assert_eq!(fetched.body, reply.body);
}

#[tokio::test]
async fn invalid_blockers_are_rejected() {
    let w = world().await;
    let stranger = uuid::Uuid::now_v7().to_string();
    for body in [
        json!({"project_id": w.project, "description": "   "}),
        json!({"project_id": w.project, "description": "x", "accountable": {"id": stranger}}),
        json!({"project_id": w.project, "description": "x", "blocked_actions": []}),
        json!({"project_id": stranger, "description": "x"}),
        json!({"description": "x"}),
    ] {
        let reply = w.create(body.clone()).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
}

#[tokio::test]
async fn a_blocker_can_be_edited_cleared_and_reopened() {
    let w = world().await;
    let created = w.new_blocker("Old", None).await;
    let id = created.body["id"].as_str().unwrap();
    let mut body = created.body.clone();
    body["description"] = json!("New");
    body["accountable"] = json!({"id": w.bob});
    let edited = w
        .app
        .send(
            Method::PUT,
            &format!("/api/v1/blockers/{id}"),
            &[("cookie", &w.cookie), ("if-match", "\"1\"")],
            Some(body),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{:?}", edited.body);
    assert_eq!(edited.body["accountable"]["display_name"], "Bob");

    let cleared = w.patch(id, "\"2\"", true).await;
    assert_eq!(cleared.body["cleared"], true);
    let stale = w.patch(id, "\"2\"", false).await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    let reopened = w.patch(id, "\"3\"", false).await;
    assert_eq!(reopened.body["cleared"], false);
    assert_eq!(reopened.header("etag"), "\"4\"");
}

#[tokio::test]
async fn linking_shows_open_blockers_on_the_action() {
    let w = world().await;
    let action = w.new_action_in(&w.project).await;
    let created = w.new_blocker("No env", None).await;
    let id = created.body["id"].as_str().unwrap();

    let linked = w.link(id, "\"1\"", &action).await;
    assert_eq!(linked.status, StatusCode::OK, "{:?}", linked.body);
    assert_eq!(linked.header("etag"), "\"2\"");
    assert_eq!(
        linked.body["blocked_actions"],
        json!([{"id": action, "reference": "A-001"}])
    );
    let shown = w.get(&format!("/api/v1/actions/{action}")).await;
    assert_eq!(
        shown.body["blocked_by"],
        json!([{"id": id, "reference": "B-001"}])
    );

    w.patch(id, "\"2\"", true).await;
    let shown = w.get(&format!("/api/v1/actions/{action}")).await;
    assert_eq!(
        shown.body["blocked_by"],
        json!([]),
        "cleared blockers don't block"
    );

    let unlinked = w.unlink(id, "\"3\"", &action).await;
    assert_eq!(unlinked.status, StatusCode::OK);
    assert_eq!(unlinked.body["blocked_actions"], json!([]));
    assert_eq!(unlinked.body["version"], 4);
}

#[tokio::test]
async fn a_cross_project_link_is_rejected() {
    let w = world().await;
    let other = w_project(&w.app, &w.cookie, "OTHR").await;
    let elsewhere = w.new_action_in(&other).await;
    let created = w.new_blocker("No env", None).await;
    let id = created.body["id"].as_str().unwrap();
    let reply = w.link(id, "\"1\"", &elsewhere).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    let missing = w
        .app
        .send(
            Method::POST,
            &format!("/api/v1/blockers/{id}/blocked_actions"),
            &[("cookie", &w.cookie)],
            Some(json!({"action_id": elsewhere})),
        )
        .await;
    assert_eq!(missing.status, StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn removing_a_member_unassigns_their_open_blockers() {
    let w = world().await;
    let open = w.new_blocker("Open", Some(&w.bob)).await;
    let id = open.body["id"].as_str().unwrap();
    let removed = w
        .app
        .send(
            Method::DELETE,
            &format!("/api/v1/projects/{}/members/{}", w.project, w.bob),
            &[("cookie", &w.cookie), ("if-match", &w.project_etag)],
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK);
    let after = w.get(&format!("/api/v1/blockers/{id}")).await;
    assert_eq!(after.body["accountable"], Value::Null);
}

#[tokio::test]
async fn blockers_in_an_archived_project_cannot_be_changed() {
    let w = world().await;
    let action = w.new_action_in(&w.project).await;
    let created = w.new_blocker("Draft", None).await;
    let archived = w
        .app
        .send(
            Method::PATCH,
            &format!("/api/v1/projects/{}", w.project),
            &[
                ("cookie", &w.cookie),
                ("if-match", &w.project_etag),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(json!({"archived": true})),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK);
    let id = created.body["id"].as_str().unwrap();
    assert_eq!(
        w.patch(id, "\"1\"", true).await.status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        w.link(id, "\"1\"", &action).await.status,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn cleared_blockers_are_listed_only_on_request() {
    let w = world().await;
    let first = w.new_blocker("One", None).await;
    w.new_blocker("Two", None).await;
    w.patch(first.body["id"].as_str().unwrap(), "\"1\"", true)
        .await;
    let base = format!("/api/v1/blockers?project_id={}", w.project);
    let open = w.get(&base).await;
    assert_eq!(open.body["total"], 1);
    assert_eq!(open.body["items"][0]["reference"], "B-002");
    let all = w.get(&format!("{base}&include_cleared=true")).await;
    assert_eq!(all.body["total"], 2);
}

#[tokio::test]
async fn writing_needs_a_session() {
    let w = world().await;
    let reply = w
        .app
        .send(
            Method::POST,
            "/api/v1/blockers",
            &[],
            Some(json!({"project_id": w.project, "description": "x"})),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}
