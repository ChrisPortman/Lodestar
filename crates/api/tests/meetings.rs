mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

/// Ada leads WEBR and has two browser sessions; Bob is a Member; Cy is not.
struct World {
    app: App,
    ada: String,
    bob: String,
    cy: String,
    ada_tab: String,
    ada_other_tab: String,
    bob_tab: String,
    cy_tab: String,
    project: String,
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
        bob: ids[0].clone(),
        cy: ids[1].clone(),
        ada_tab,
        project: project_id,
    }
}

impl World {
    async fn lock(&self, method: Method, cookie: &str) -> Reply {
        let path = format!("/api/v1/projects/{}/meeting-lock", self.project);
        self.app
            .send(method, &path, &[("cookie", cookie)], None)
            .await
    }

    async fn end(&self, cookie: &str, body: Value) -> Reply {
        self.app
            .send(
                Method::POST,
                "/api/v1/meetings",
                &[("cookie", cookie)],
                Some(body),
            )
            .await
    }

    fn meeting(&self, attendees: &[&str], comments: &[&str]) -> Value {
        let comments: Vec<_> = comments.iter().map(|b| json!({"body": b})).collect();
        json!({"project_id": self.project, "attendee_ids": attendees, "comments": comments})
    }

    async fn list(&self, cookie: &str) -> Reply {
        self.app
            .get(
                &format!("/api/v1/meetings?project_id={}", self.project),
                cookie,
            )
            .await
    }
}

#[tokio::test]
async fn the_leader_ends_a_meeting_and_members_read_it() {
    let w = world().await;
    let lock = w.lock(Method::POST, &w.ada_tab).await;
    let project_comment = w
        .app
        .send(
            Method::POST,
            "/api/v1/comments",
            &[("cookie", &w.bob_tab)],
            Some(json!({"project_id": w.project, "target_type": "project",
                        "target_id": w.project, "body": "Launch moved"})),
        )
        .await;
    assert_eq!(project_comment.status, StatusCode::CREATED);

    let created = w
        .end(&w.ada_tab, w.meeting(&[&w.bob], &["Good progress"]))
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.body["id"].as_str().unwrap();
    assert_eq!(created.header("location"), format!("/api/v1/meetings/{id}"));
    assert_eq!(created.body["project_id"], w.project.as_str());
    assert_eq!(created.body["leader"]["id"], w.ada.as_str());
    let attendees: Vec<_> = created.body["attendees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(attendees, ["Ada", "Bob"], "the leader is added first");
    assert_eq!(created.body["started_at"], lock.body["acquired_at"]);
    assert_eq!(created.body["version"], 1);
    assert_eq!(
        created.body["minutes"]["meeting_comments"],
        json!(["Launch moved", "Good progress"])
    );
    assert_eq!(created.body["minutes"]["attendees"], json!(["Ada", "Bob"]));
    assert_eq!(
        w.lock(Method::GET, &w.ada_tab).await.status,
        StatusCode::NOT_FOUND,
        "the lock is released"
    );

    let read = w
        .app
        .get(&format!("/api/v1/meetings/{id}"), &w.bob_tab)
        .await;
    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(read.body, created.body);

    let listed = w.list(&w.bob_tab).await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.body["total"], 1);
    let item = &listed.body["items"][0];
    assert_eq!(item["id"], id);
    let keys: Vec<_> = item.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "attendees",
            "ended_at",
            "id",
            "leader",
            "project_id",
            "started_at",
            "version"
        ],
        "listed without minutes"
    );
}

#[tokio::test]
async fn ending_needs_a_session_holding_the_lock() {
    let w = world().await;
    let body = w.meeting(&[&w.bob], &[]);
    assert_eq!(
        w.end("", body.clone()).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        w.end(&w.ada_tab, body.clone()).await.status,
        StatusCode::CONFLICT,
        "no lock"
    );
    w.lock(Method::POST, &w.ada_tab).await;
    assert_eq!(
        w.end(&w.ada_other_tab, body.clone()).await.status,
        StatusCode::CONFLICT,
        "another session's lock"
    );
    assert_eq!(
        w.end(&w.bob_tab, body).await.status,
        StatusCode::CONFLICT,
        "not the holder"
    );
}

#[tokio::test]
async fn attendees_must_be_given_and_be_members() {
    let w = world().await;
    w.lock(Method::POST, &w.ada_tab).await;
    for body in [
        w.meeting(&[], &[]),
        w.meeting(&[&w.bob, &w.cy], &[]),
        json!({"project_id": w.project, "comments": []}),
    ] {
        assert_eq!(
            w.end(&w.ada_tab, body).await.status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}

#[tokio::test]
async fn a_failed_end_keeps_the_lock_and_creates_no_meeting() {
    let w = world().await;
    w.lock(Method::POST, &w.ada_tab).await;
    let too_long = "x".repeat(4001);
    let failed = w
        .end(&w.ada_tab, w.meeting(&[&w.bob], &["Fine", &too_long]))
        .await;
    assert_eq!(failed.status, StatusCode::UNPROCESSABLE_ENTITY);

    let lock = w.lock(Method::GET, &w.ada_tab).await;
    assert_eq!(lock.status, StatusCode::OK);
    assert_eq!(lock.body["held_by_you"], true);
    assert_eq!(w.list(&w.ada_tab).await.body["total"], 0);
    let comments = w
        .app
        .get(
            &format!("/api/v1/comments?project_id={}", w.project),
            &w.ada_tab,
        )
        .await;
    assert_eq!(comments.body["total"], 0);
}

#[tokio::test]
async fn meeting_comments_can_be_edited_and_added_without_changing_the_minutes() {
    let w = world().await;
    w.lock(Method::POST, &w.ada_tab).await;
    let created = w
        .end(&w.ada_tab, w.meeting(&[&w.bob], &["Agreed scope"]))
        .await;
    let id = created.body["id"].as_str().unwrap();

    let comments = w
        .app
        .get(
            &format!(
                "/api/v1/comments?project_id={}&target_type=meeting&target_id={id}",
                w.project
            ),
            &w.ada_tab,
        )
        .await;
    assert_eq!(comments.body["total"], 1);
    let comment = &comments.body["items"][0];
    assert_eq!(comment["body"], "Agreed scope");
    let edited = w
        .app
        .send(
            Method::PUT,
            &format!("/api/v1/comments/{}", comment["id"].as_str().unwrap()),
            &[("cookie", &w.ada_tab), ("if-match", "\"1\"")],
            Some(json!({"body": "Scope cut"})),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK);
    let added = w
        .app
        .send(
            Method::POST,
            "/api/v1/comments",
            &[("cookie", &w.bob_tab)],
            Some(json!({"project_id": w.project, "target_type": "meeting",
                        "target_id": id, "body": "Added later"})),
        )
        .await;
    assert_eq!(added.status, StatusCode::CREATED);

    let read = w
        .app
        .get(&format!("/api/v1/meetings/{id}"), &w.bob_tab)
        .await;
    assert_eq!(read.body["minutes"], created.body["minutes"]);
}

#[tokio::test]
async fn only_members_read_meetings() {
    let w = world().await;
    w.lock(Method::POST, &w.ada_tab).await;
    let created = w.end(&w.ada_tab, w.meeting(&[&w.bob], &[])).await;
    let id = created.body["id"].as_str().unwrap();

    assert_eq!(w.list(&w.cy_tab).await.status, StatusCode::FORBIDDEN);
    assert_eq!(w.list("").await.status, StatusCode::UNAUTHORIZED);
    let path = format!("/api/v1/meetings/{id}");
    assert_eq!(
        w.app.get(&path, &w.cy_tab).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        w.app
            .get(
                "/api/v1/meetings/0192a2b0-0000-7000-8000-000000000000",
                &w.bob_tab
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn meetings_are_listed_newest_first() {
    let w = world().await;
    let mut ids = Vec::new();
    for _ in 0..2 {
        w.lock(Method::POST, &w.ada_tab).await;
        let created = w.end(&w.ada_tab, w.meeting(&[&w.bob], &[])).await;
        ids.push(created.body["id"].as_str().unwrap().to_owned());
    }
    let listed = w.list(&w.ada_tab).await;
    let listed: Vec<_> = listed.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed, [ids[1].as_str(), ids[0].as_str()]);
}

#[tokio::test]
async fn meetings_cannot_be_changed_or_deleted() {
    let w = world().await;
    w.lock(Method::POST, &w.ada_tab).await;
    let created = w.end(&w.ada_tab, w.meeting(&[&w.bob], &[])).await;
    let path = format!("/api/v1/meetings/{}", created.body["id"].as_str().unwrap());
    for method in [Method::PUT, Method::PATCH, Method::DELETE] {
        let reply = w
            .app
            .send(method, &path, &[("cookie", &w.ada_tab)], Some(json!({})))
            .await;
        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(reply.header("allow"), "GET,HEAD");
    }
}
