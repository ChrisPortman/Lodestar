mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

const PROBLEM: &str = "application/problem+json";

/// Ada's Project WEBR with Bob as a Member and Carol as an outsider, plus a
/// Risk in it.
struct World {
    app: App,
    ada: String,
    bob: String,
    carol: String,
    project: String,
    risk: String,
}

async fn post(app: &App, cookie: &str, path: &str, body: Value) -> Value {
    let reply = app
        .send(Method::POST, path, &[("cookie", cookie)], Some(body))
        .await;
    assert!(reply.status.is_success(), "{path}: {:?}", reply.body);
    reply.body
}

fn id(body: &Value) -> String {
    body["id"].as_str().unwrap().to_owned()
}

async fn session(app: &App, person: &str) -> String {
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
    let ada = app.first_person("Ada").await.cookie();
    let people = json!({"display_name": "Bob"});
    let bob = id(&post(&app, &ada, "/api/v1/people", people).await);
    let people = json!({"display_name": "Carol"});
    let carol = id(&post(&app, &ada, "/api/v1/people", people).await);
    let project = id(&post(
        &app,
        &ada,
        "/api/v1/projects",
        json!({"name": "Website", "project_code": "WEBR"}),
    )
    .await);
    let added = app
        .send(
            Method::POST,
            &format!("/api/v1/projects/{project}/members"),
            &[("cookie", &ada), ("if-match", "\"1\"")],
            Some(json!({"person_id": bob})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK);
    let risk = id(&post(
        &app,
        &ada,
        "/api/v1/risks",
        json!({"project_id": project, "description": "Late", "likelihood": 2, "impact": 3}),
    )
    .await);
    let bob = session(&app, &bob).await;
    let carol = session(&app, &carol).await;
    World {
        app,
        ada,
        bob,
        carol,
        project,
        risk,
    }
}

async fn create(w: &World, cookie: &str, body: Value) -> Reply {
    w.app
        .send(
            Method::POST,
            "/api/v1/comments",
            &[("cookie", cookie)],
            Some(body),
        )
        .await
}

async fn comment_on_risk(w: &World, cookie: &str, text: &str) -> Value {
    let reply = create(
        w,
        cookie,
        json!({"project_id": w.project, "target_type": "risk", "target_id": w.risk, "body": text}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    reply.body
}

async fn put(w: &World, cookie: &str, id: &str, etag: &str, body: Value) -> Reply {
    w.app
        .send(
            Method::PUT,
            &format!("/api/v1/comments/{id}"),
            &[("cookie", cookie), ("if-match", etag)],
            Some(body),
        )
        .await
}

async fn patch(w: &World, cookie: &str, id: &str, etag: &str, body: Value) -> Reply {
    w.app
        .send(
            Method::PATCH,
            &format!("/api/v1/comments/{id}"),
            &[
                ("cookie", cookie),
                ("if-match", etag),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(body),
        )
        .await
}

#[tokio::test]
async fn a_member_comments_on_a_risk_and_it_can_be_fetched() {
    let w = world().await;
    let reply = create(
        &w,
        &w.bob,
        json!({"project_id": w.project, "target_type": "risk", "target_id": w.risk,
               "body": " **Late** <b>again</b>\n"}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    let id = id(&reply.body);
    assert_eq!(reply.header("location"), format!("/api/v1/comments/{id}"));
    assert_eq!(reply.header("etag"), "\"1\"");
    assert_eq!(reply.body["target_type"], "risk");
    assert_eq!(reply.body["target_id"], w.risk.as_str());
    assert_eq!(reply.body["author"]["display_name"], "Bob");
    assert_eq!(reply.body["body"], " **Late** <b>again</b>\n");
    assert!(reply.body["created_at"].is_string());
    assert_eq!(reply.body["edited_at"], Value::Null);
    assert_eq!(reply.body["deleted"], false);

    let fetched = w.app.get(&format!("/api/v1/comments/{id}"), "").await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.body, reply.body);

    let risk = w.app.get(&format!("/api/v1/risks/{}", w.risk), "").await;
    assert_eq!(risk.body["version"], 1, "commenting leaves the Risk alone");
}

#[tokio::test]
async fn every_kind_of_target_can_be_commented_on() {
    let w = world().await;
    let action = id(&post(
        &w.app,
        &w.ada,
        "/api/v1/actions",
        json!({"project_id": w.project, "description": "Ship"}),
    )
    .await);
    let blocker = id(&post(
        &w.app,
        &w.ada,
        "/api/v1/blockers",
        json!({"project_id": w.project, "description": "Waiting"}),
    )
    .await);
    for (kind, target) in [
        ("project", &w.project),
        ("action", &action),
        ("risk", &w.risk),
        ("blocker", &blocker),
    ] {
        let reply = create(
            &w,
            &w.ada,
            json!({"project_id": w.project, "target_type": kind, "target_id": target, "body": "Hi"}),
        )
        .await;
        assert_eq!(
            reply.status,
            StatusCode::CREATED,
            "{kind}: {:?}",
            reply.body
        );
    }
}

#[tokio::test]
async fn invalid_comments_are_a_422_problem() {
    let w = world().await;
    let other = id(&post(
        &w.app,
        &w.ada,
        "/api/v1/projects",
        json!({"name": "Other", "project_code": "OTHR"}),
    )
    .await);
    let p = &w.project;
    let r = &w.risk;
    for body in [
        json!({"project_id": p, "target_type": "risk", "target_id": r, "body": ""}),
        json!({"project_id": p, "target_type": "risk", "target_id": r, "body": " \n "}),
        json!({"project_id": p, "target_type": "risk", "target_id": r, "body": "x".repeat(4001)}),
        json!({"project_id": p, "target_type": "risk", "target_id": r}),
        json!({"project_id": p, "target_type": "risk", "body": "Hi"}),
        json!({"project_id": p, "target_type": "thing", "target_id": r, "body": "Hi"}),
        json!({"project_id": p, "target_type": "action", "target_id": r, "body": "Hi"}),
        json!({"project_id": p, "target_type": "project", "target_id": other, "body": "Hi"}),
        json!({"project_id": other, "target_type": "risk", "target_id": r, "body": "Hi"}),
        json!({"project_id": p, "target_type": "risk", "target_id": r, "body": "Hi", "deleted": false}),
        json!({"project_id": "00000000-0000-7000-8000-000000000000", "target_type": "risk",
               "target_id": r, "body": "Hi"}),
    ] {
        let reply = create(&w, &w.ada, body.clone()).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(reply.header("content-type"), PROBLEM);
    }
    let longest = json!({"project_id": p, "target_type": "risk", "target_id": r,
                         "body": "x".repeat(4000)});
    assert_eq!(
        create(&w, &w.ada, longest).await.status,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn commenting_needs_a_session_and_membership() {
    let w = world().await;
    let body =
        json!({"project_id": w.project, "target_type": "risk", "target_id": w.risk, "body": "Hi"});
    assert_eq!(
        create(&w, "", body.clone()).await.status,
        StatusCode::UNAUTHORIZED
    );
    let reply = create(&w, &w.carol, body).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.header("content-type"), PROBLEM);
}

#[tokio::test]
async fn the_author_edits_their_comment() {
    let w = world().await;
    let created = comment_on_risk(&w, &w.bob, "Hi").await;
    let id = id(&created);

    let mut whole = created.clone();
    whole["body"] = json!("Hello");
    let reply = put(&w, &w.bob, &id, "\"1\"", whole).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.header("etag"), "\"2\"");
    assert_eq!(reply.body["body"], "Hello");
    assert_eq!(reply.body["created_at"], created["created_at"]);
    assert!(reply.body["edited_at"].is_string());

    let stale = put(&w, &w.bob, &id, "\"1\"", json!({"body": "Again"})).await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    let no_match = w
        .app
        .send(
            Method::PUT,
            &format!("/api/v1/comments/{id}"),
            &[("cookie", &w.bob)],
            Some(json!({"body": "Again"})),
        )
        .await;
    assert_eq!(no_match.status, StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn read_only_fields_must_match_on_put() {
    let w = world().await;
    let created = comment_on_risk(&w, &w.bob, "Hi").await;
    let id = id(&created);
    for (field, value) in [
        ("target_type", json!("project")),
        ("target_id", json!(w.project)),
        (
            "author",
            json!({"id": "00000000-0000-7000-8000-000000000000", "display_name": "X"}),
        ),
        ("created_at", json!("2020-01-01T00:00:00Z")),
        ("edited_at", json!("2020-01-01T00:00:00Z")),
        ("deleted", json!(true)),
    ] {
        let mut body = created.clone();
        body["body"] = json!("Changed");
        body[field] = value;
        let reply = put(&w, &w.bob, &id, "\"1\"", body).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{field}");
    }
}

#[tokio::test]
async fn only_the_author_can_edit_or_delete() {
    let w = world().await;
    let id = id(&comment_on_risk(&w, &w.bob, "Hi").await);
    let edit = put(&w, &w.ada, &id, "\"1\"", json!({"body": "Mine"})).await;
    assert_eq!(edit.status, StatusCode::FORBIDDEN);
    let delete = patch(&w, &w.ada, &id, "\"1\"", json!({"deleted": true})).await;
    assert_eq!(delete.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_deleted_comment_hides_its_text_and_stays_deleted() {
    let w = world().await;
    let id = id(&comment_on_risk(&w, &w.bob, "Secret").await);
    let undelete = patch(&w, &w.bob, &id, "\"1\"", json!({"deleted": false})).await;
    assert_eq!(undelete.status, StatusCode::UNPROCESSABLE_ENTITY);

    let reply = patch(&w, &w.bob, &id, "\"1\"", json!({"deleted": true})).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.header("etag"), "\"2\"");
    assert_eq!(reply.body["deleted"], true);
    assert_eq!(reply.body["body"], "");
    let fetched = w.app.get(&format!("/api/v1/comments/{id}"), "").await;
    assert_eq!(fetched.body["body"], "");

    let edit = put(&w, &w.bob, &id, "\"2\"", json!({"body": "Back"})).await;
    assert_eq!(edit.status, StatusCode::CONFLICT);
    let delete = w
        .app
        .send(
            Method::DELETE,
            &format!("/api/v1/comments/{id}"),
            &[("cookie", &w.bob)],
            None,
        )
        .await;
    assert_eq!(delete.status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn comments_are_listed_oldest_first_with_deleted_ones() {
    let w = world().await;
    let first = id(&comment_on_risk(&w, &w.ada, "First").await);
    comment_on_risk(&w, &w.bob, "Second").await;
    let on_project = create(
        &w,
        &w.ada,
        json!({"project_id": w.project, "target_type": "project", "target_id": w.project, "body": "Third"}),
    )
    .await;
    assert_eq!(on_project.status, StatusCode::CREATED);
    patch(&w, &w.ada, &first, "\"1\"", json!({"deleted": true})).await;

    let bodies = |reply: &Reply| -> Vec<String> {
        reply.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["body"].as_str().unwrap().to_owned())
            .collect()
    };
    let all = w
        .app
        .get(&format!("/api/v1/comments?project_id={}", w.project), "")
        .await;
    assert_eq!(all.status, StatusCode::OK);
    assert_eq!(all.body["total"], 3);
    assert_eq!(bodies(&all), ["", "Second", "Third"]);

    let on_risk = w
        .app
        .get(
            &format!(
                "/api/v1/comments?project_id={}&target_type=risk&target_id={}&limit=1&offset=1",
                w.project, w.risk
            ),
            "",
        )
        .await;
    assert_eq!(on_risk.body["total"], 2);
    assert_eq!(bodies(&on_risk), ["Second"]);

    for query in [
        String::new(),
        format!("project_id={}&target_type=risk", w.project),
        format!("project_id={}&limit=0", w.project),
    ] {
        let reply = w.app.get(&format!("/api/v1/comments?{query}"), "").await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
    }
}

#[tokio::test]
async fn nothing_changes_on_an_archived_project() {
    let w = world().await;
    let id = id(&comment_on_risk(&w, &w.ada, "Hi").await);
    let archived = w
        .app
        .send(
            Method::PATCH,
            &format!("/api/v1/projects/{}", w.project),
            &[
                ("cookie", &w.ada),
                ("if-match", "\"2\""),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(json!({"archived": true})),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK, "{:?}", archived.body);

    let body =
        json!({"project_id": w.project, "target_type": "risk", "target_id": w.risk, "body": "Hi"});
    assert_eq!(create(&w, &w.ada, body).await.status, StatusCode::CONFLICT);
    let edit = put(&w, &w.ada, &id, "\"1\"", json!({"body": "Hello"})).await;
    assert_eq!(edit.status, StatusCode::CONFLICT);
    let delete = patch(&w, &w.ada, &id, "\"1\"", json!({"deleted": true})).await;
    assert_eq!(delete.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn the_export_holds_every_comment_with_its_text_and_history() {
    let w = world().await;
    let kept = id(&comment_on_risk(&w, &w.bob, "Kept").await);
    let gone = id(&comment_on_risk(&w, &w.bob, "Gone").await);
    patch(&w, &w.bob, &gone, "\"1\"", json!({"deleted": true})).await;

    let reply = w
        .app
        .get(
            &format!("/api/v1/projects/{}/export?include_history=true", w.project),
            &w.ada,
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    let comments = reply.body["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0]["id"], kept.as_str());
    assert_eq!(comments[0]["body"], "Kept");
    assert_eq!(comments[1]["body"], "Gone");
    assert_eq!(comments[1]["deleted"], true);
    assert!(
        reply.body["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["display_name"] == "Bob")
    );

    let history = reply.body["change_history"].as_array().unwrap();
    let comment_entries: Vec<_> = history
        .iter()
        .filter(|e| e["entity_type"] == "comment")
        .map(|e| e["change_kind"].as_str().unwrap())
        .collect();
    assert_eq!(comment_entries, ["created", "created", "updated"]);
    let risk_entries = history
        .iter()
        .filter(|e| e["entity_type"] == "risk")
        .count();
    assert_eq!(risk_entries, 1);
}
