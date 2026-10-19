mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

const PROBLEM: &str = "application/problem+json";

/// An app with one Person, and their session cookie.
async fn with_person() -> (App, String) {
    let app = app().await;
    let cookie = app.first_person("Ada").await.cookie();
    (app, cookie)
}

async fn create(app: &App, cookie: &str, body: Value) -> Reply {
    app.send(
        Method::POST,
        "/api/v1/projects",
        &[("cookie", cookie)],
        Some(body),
    )
    .await
}

async fn put(app: &App, cookie: &str, id: &str, etag: &str, body: Value) -> Reply {
    app.send(
        Method::PUT,
        &format!("/api/v1/projects/{id}"),
        &[("cookie", cookie), ("if-match", etag)],
        Some(body),
    )
    .await
}

async fn patch(app: &App, cookie: &str, id: &str, etag: &str, body: Value) -> Reply {
    app.send(
        Method::PATCH,
        &format!("/api/v1/projects/{id}"),
        &[
            ("cookie", cookie),
            ("if-match", etag),
            ("content-type", "application/merge-patch+json"),
        ],
        Some(body),
    )
    .await
}

fn names(reply: &Reply) -> Vec<String> {
    reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn creating_a_project_makes_the_acting_person_its_leader() {
    let (app, cookie) = with_person().await;
    let reply = create(
        &app,
        &cookie,
        json!({"name": " Website Rebuild ", "description": "New site", "project_code": "WEBR"}),
    )
    .await;

    assert_eq!(reply.status, StatusCode::CREATED);
    let id = reply.body["id"].as_str().unwrap();
    assert_eq!(reply.header("location"), format!("/api/v1/projects/{id}"));
    assert_eq!(reply.header("etag"), "\"1\"");
    assert_eq!(reply.body["name"], "Website Rebuild");
    assert_eq!(reply.body["description"], "New site");
    assert_eq!(reply.body["project_code"], "WEBR");
    assert_eq!(reply.body["archived"], false);
    assert_eq!(reply.body["project_leader"]["display_name"], "Ada");
    assert_eq!(reply.body["version"], 1);

    let fetched = app.get(&format!("/api/v1/projects/{id}"), "").await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.body, reply.body);
}

#[tokio::test]
async fn creating_needs_a_session() {
    let app = app().await;
    let reply = create(&app, "", json!({"name": "Web", "project_code": "WEBR"})).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_projects_are_a_422_problem() {
    let (app, cookie) = with_person().await;
    for body in [
        json!({"name": "", "project_code": "WEBR"}),
        json!({"name": "x".repeat(101), "project_code": "WEBR"}),
        json!({"name": "Web", "project_code": "webr"}),
        json!({"name": "Web", "project_code": "W"}),
        json!({"name": "Web", "project_code": "WEBR", "description": "  "}),
        json!({"name": "Web"}),
        json!({"name": "Web", "project_code": "WEBR", "version": 1}),
        json!({"name": "Web", "project_code": "WEBR", "archived": true}),
    ] {
        let reply = create(&app, &cookie, body.clone()).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(reply.header("content-type"), PROBLEM);
    }
}

#[tokio::test]
async fn a_project_code_in_use_is_a_409_problem() {
    let (app, cookie) = with_person().await;
    let first = create(
        &app,
        &cookie,
        json!({"name": "Web", "project_code": "WEBR"}),
    )
    .await;
    assert_eq!(first.status, StatusCode::CREATED);
    let reply = create(
        &app,
        &cookie,
        json!({"name": "Other", "project_code": "WEBR"}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.header("content-type"), PROBLEM);
}

#[tokio::test]
async fn code_suggestions_skip_codes_in_use() {
    let (app, cookie) = with_person().await;
    let path = "/api/v1/projects/code_suggestion?name=Website";
    assert_eq!(app.get(path, "").await.body["project_code"], "WEBS");
    create(
        &app,
        &cookie,
        json!({"name": "Website", "project_code": "WEBS"}),
    )
    .await;
    let reply = app.get(path, "").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["project_code"], "WEBS1");

    let none = app.get("/api/v1/projects/code_suggestion?name=1", "").await;
    assert_eq!(none.body["project_code"], Value::Null);
}

#[tokio::test]
async fn put_renames_and_redescribes() {
    let (app, cookie) = with_person().await;
    let created = create(
        &app,
        &cookie,
        json!({"name": "Web", "description": "Old", "project_code": "WEBR"}),
    )
    .await;
    let id = created.body["id"].as_str().unwrap();

    let reply = put(
        &app,
        &cookie,
        id,
        "\"1\"",
        json!({"name": "Website", "description": null}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["name"], "Website");
    assert_eq!(reply.body["description"], Value::Null);
    assert_eq!(reply.body["version"], 2);
    assert_eq!(reply.header("etag"), "\"2\"");

    // Every writable field is required; read-only fields may be echoed back.
    let missing = put(&app, &cookie, id, "\"2\"", json!({"name": "Web"})).await;
    assert_eq!(missing.status, StatusCode::UNPROCESSABLE_ENTITY);
    let mut echoed = reply.body.clone();
    echoed["name"] = json!("Web 2");
    let echoed = put(&app, &cookie, id, "\"2\"", echoed).await;
    assert_eq!(echoed.status, StatusCode::OK);

    let stale = put(
        &app,
        &cookie,
        id,
        "\"1\"",
        json!({"name": "X", "description": null}),
    )
    .await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
}

#[tokio::test]
async fn the_project_code_cannot_be_changed_by_put() {
    let (app, cookie) = with_person().await;
    let created = create(
        &app,
        &cookie,
        json!({"name": "Web", "project_code": "WEBR"}),
    )
    .await;
    let id = created.body["id"].as_str().unwrap();

    let reply = put(
        &app,
        &cookie,
        id,
        "\"1\"",
        json!({"name": "Web", "description": null, "project_code": "NEWC"}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.header("content-type"), PROBLEM);

    let archived = put(
        &app,
        &cookie,
        id,
        "\"1\"",
        json!({"name": "Web", "description": null, "archived": true}),
    )
    .await;
    assert_eq!(archived.status, StatusCode::UNPROCESSABLE_ENTITY);

    let now = app.get(&format!("/api/v1/projects/{id}"), "").await;
    assert_eq!(now.body["project_code"], "WEBR");
    assert_eq!(now.body["archived"], false);
    assert_eq!(now.body["version"], 1);
}

#[tokio::test]
async fn an_archived_project_is_read_only_until_unarchived() {
    let (app, cookie) = with_person().await;
    let created = create(
        &app,
        &cookie,
        json!({"name": "Web", "project_code": "WEBR"}),
    )
    .await;
    let id = created.body["id"].as_str().unwrap();

    let archived = patch(&app, &cookie, id, "\"1\"", json!({"archived": true})).await;
    assert_eq!(archived.status, StatusCode::OK);
    assert_eq!(archived.body["archived"], true);
    assert_eq!(archived.header("etag"), "\"2\"");

    let edit = put(
        &app,
        &cookie,
        id,
        "\"2\"",
        json!({"name": "Website", "description": null}),
    )
    .await;
    assert_eq!(edit.status, StatusCode::CONFLICT);
    assert_eq!(edit.header("content-type"), PROBLEM);
    assert_eq!(
        app.get(&format!("/api/v1/projects/{id}"), "").await.body["name"],
        "Web"
    );

    let unarchived = patch(&app, &cookie, id, "\"2\"", json!({"archived": false})).await;
    assert_eq!(unarchived.status, StatusCode::OK);
    let edit = put(
        &app,
        &cookie,
        id,
        "\"3\"",
        json!({"name": "Website", "description": null}),
    )
    .await;
    assert_eq!(edit.status, StatusCode::OK);
}

#[tokio::test]
async fn patch_changes_only_the_archived_flag() {
    let (app, cookie) = with_person().await;
    let created = create(
        &app,
        &cookie,
        json!({"name": "Web", "project_code": "WEBR"}),
    )
    .await;
    let id = created.body["id"].as_str().unwrap();

    for body in [
        json!({"name": "X"}),
        json!({"archived": true, "name": "X"}),
        json!({}),
    ] {
        let reply = patch(&app, &cookie, id, "\"1\"", body.clone()).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let stale = patch(&app, &cookie, id, "\"9\"", json!({"archived": true})).await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    let no_session = patch(&app, "", id, "\"1\"", json!({"archived": true})).await;
    assert_eq!(no_session.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn archived_projects_are_listed_only_when_asked() {
    let (app, cookie) = with_person().await;
    let web = create(
        &app,
        &cookie,
        json!({"name": "Web", "project_code": "WEBR"}),
    )
    .await;
    create(
        &app,
        &cookie,
        json!({"name": "Alpha", "project_code": "ALPH"}),
    )
    .await;
    let id = web.body["id"].as_str().unwrap();
    patch(&app, &cookie, id, "\"1\"", json!({"archived": true})).await;

    let default = app.get("/api/v1/projects", "").await;
    assert_eq!(default.status, StatusCode::OK);
    assert_eq!(names(&default), ["Alpha"]);
    assert_eq!(default.body["total"], 1);

    let all = app
        .get("/api/v1/projects?include_archived=true&sort=name", "")
        .await;
    assert_eq!(names(&all), ["Alpha", "Web"]);
    assert_eq!(all.body["limit"], 50);

    let bad = app.get("/api/v1/projects?limit=0", "").await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn projects_are_never_deleted() {
    let (app, cookie) = with_person().await;
    let created = create(
        &app,
        &cookie,
        json!({"name": "Web", "project_code": "WEBR"}),
    )
    .await;
    let id = created.body["id"].as_str().unwrap();
    for path in [
        format!("/api/v1/projects/{id}"),
        "/api/v1/projects".to_owned(),
    ] {
        let reply = app
            .send(Method::DELETE, &path, &[("cookie", &cookie)], None)
            .await;
        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    }
}

#[tokio::test]
async fn an_unknown_project_is_404() {
    let app = app().await;
    let reply = app
        .get("/api/v1/projects/0192f0c4-0000-7000-8000-000000000000", "")
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}
