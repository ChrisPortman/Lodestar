mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, app};

/// Ada's Project WEBR with Bob as a Member, one closed Action assigned to
/// Bob, a Risk and a Blocker blocking the Action; then archived.
struct World {
    app: App,
    cookie: String,
    ada: String,
    bob: String,
    project: String,
}

async fn post(app: &App, cookie: &str, path: &str, body: Value) -> Value {
    let reply = app
        .send(Method::POST, path, &[("cookie", cookie)], Some(body))
        .await;
    assert!(reply.status.is_success(), "{path}: {:?}", reply.body);
    reply.body
}

async fn world() -> World {
    let app = app().await;
    let first = app.first_person("Ada").await;
    let cookie = first.cookie();
    let ada = first.body["id"].as_str().unwrap().to_owned();
    let bob = post(
        &app,
        &cookie,
        "/api/v1/people",
        json!({"display_name": "Bob"}),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let project = post(
        &app,
        &cookie,
        "/api/v1/projects",
        json!({"name": "Website", "project_code": "WEBR"}),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let added = app
        .send(
            Method::POST,
            &format!("/api/v1/projects/{project}/members"),
            &[("cookie", &cookie), ("if-match", "\"1\"")],
            Some(json!({"person_id": bob})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK);
    let action = post(
        &app,
        &cookie,
        "/api/v1/actions",
        json!({"project_id": project, "description": "Ship", "assignee": {"id": bob}}),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    post(
        &app,
        &cookie,
        "/api/v1/risks",
        json!({"project_id": project, "description": "Late", "likelihood": 2, "impact": 3}),
    )
    .await;
    let blocker = post(
        &app,
        &cookie,
        "/api/v1/blockers",
        json!({"project_id": project, "description": "Waiting"}),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let linked = app
        .send(
            Method::POST,
            &format!("/api/v1/blockers/{blocker}/blocked_actions"),
            &[("cookie", &cookie), ("if-match", "\"1\"")],
            Some(json!({"action_id": action})),
        )
        .await;
    assert!(linked.status.is_success(), "{:?}", linked.body);
    let closed = app
        .send(
            Method::PATCH,
            &format!("/api/v1/actions/{action}"),
            &[
                ("cookie", &cookie),
                ("if-match", "\"1\""),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(json!({"closed": true})),
        )
        .await;
    assert_eq!(closed.status, StatusCode::OK, "{:?}", closed.body);
    let archived = app
        .send(
            Method::PATCH,
            &format!("/api/v1/projects/{project}"),
            &[
                ("cookie", &cookie),
                ("if-match", "\"2\""),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(json!({"archived": true})),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK, "{:?}", archived.body);
    World {
        app,
        cookie,
        ada,
        bob,
        project,
    }
}

#[tokio::test]
async fn export_without_history_holds_the_whole_project_as_an_attachment() {
    let w = world().await;
    let reply = w
        .app
        .get(&format!("/api/v1/projects/{}/export", w.project), &w.cookie)
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    let disposition = reply.header("content-disposition");
    let today = chrono::Utc::now().date_naive();
    assert_eq!(
        disposition,
        format!("attachment; filename=\"WEBR-{today}.json\"")
    );
    let body = &reply.body;
    assert_eq!(body["format"], "lodestar.project-export");
    assert_eq!(body["format_version"], 1);
    assert!(body["exported_at"].is_string());
    assert_eq!(body["exported_by"]["id"], w.ada);
    assert_eq!(body["exported_by"]["display_name"], "Ada");
    assert_eq!(body["project"]["project_code"], "WEBR");
    assert_eq!(body["project"]["archived"], true);
    let members: Vec<&str> = body["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(members, ["Ada", "Bob"]);
    assert_eq!(body["actions"][0]["reference"], "A-001");
    assert_eq!(body["actions"][0]["closed"], true);
    assert_eq!(body["actions"][0]["assignee"]["id"], w.bob);
    assert_eq!(body["risks"][0]["reference"], "R-001");
    assert_eq!(body["blockers"][0]["reference"], "B-001");
    assert_eq!(
        body["blockers"][0]["blocked_actions"][0]["reference"],
        "A-001"
    );
    let people: Vec<&str> = body["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(people, ["Ada", "Bob"]);
    assert!(body.get("change_history").is_none());
}

#[tokio::test]
async fn export_with_history_lists_every_change_in_time_order() {
    let w = world().await;
    let reply = w
        .app
        .get(
            &format!("/api/v1/projects/{}/export?include_history=true", w.project),
            &w.cookie,
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    let history = reply.body["change_history"].as_array().unwrap();
    let kinds: Vec<(&str, &str, u64)> = history
        .iter()
        .map(|e| {
            (
                e["entity_type"].as_str().unwrap(),
                e["change_kind"].as_str().unwrap(),
                e["version"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("project", "created", 1),
            ("project", "updated", 2),
            ("action", "created", 1),
            ("risk", "created", 1),
            ("blocker", "created", 1),
            ("blocker", "updated", 2),
            ("action", "updated", 2),
            ("project", "updated", 3),
        ]
    );
    assert!(history.iter().all(|e| e["acting_person_id"] == w.ada));
    assert!(history[0]["before"].is_null());
    assert_eq!(history[7]["after"]["archived"], true);
}

async fn send(w: &World, method: Method, path: &str, cookie: &str, etag: &str, body: Value) {
    let ct = if method == Method::PATCH {
        "application/merge-patch+json"
    } else {
        "application/json"
    };
    let reply = w
        .app
        .send(
            method,
            path,
            &[("cookie", cookie), ("if-match", etag), ("content-type", ct)],
            Some(body),
        )
        .await;
    assert!(reply.status.is_success(), "{path}: {:?}", reply.body);
}

#[tokio::test]
async fn people_include_history_actors_only_when_history_is_exported() {
    let w = world().await;
    let project_path = format!("/api/v1/projects/{}", w.project);
    send(
        &w,
        Method::PATCH,
        &project_path,
        &w.cookie,
        "\"3\"",
        json!({"archived": false}),
    )
    .await;
    let carol = post(
        &w.app,
        &w.cookie,
        "/api/v1/people",
        json!({"display_name": "Carol"}),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    send(
        &w,
        Method::POST,
        &format!("{project_path}/members"),
        &w.cookie,
        "\"4\"",
        json!({"person_id": carol}),
    )
    .await;
    let carol_cookie = w
        .app
        .send(
            Method::POST,
            "/api/v1/session",
            &[],
            Some(json!({"person_id": carol})),
        )
        .await
        .cookie();
    post(
        &w.app,
        &carol_cookie,
        "/api/v1/risks",
        json!({"project_id": w.project, "description": "Budget", "likelihood": 1, "impact": 1}),
    )
    .await;
    let removed = w
        .app
        .send(
            Method::DELETE,
            &format!("{project_path}/members/{carol}"),
            &[("cookie", &w.cookie), ("if-match", "\"5\"")],
            None,
        )
        .await;
    assert!(removed.status.is_success(), "{:?}", removed.body);

    let names = |body: &Value| -> Vec<String> {
        body["people"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["display_name"].as_str().unwrap().to_owned())
            .collect()
    };
    let without = w
        .app
        .get(&format!("{project_path}/export"), &w.cookie)
        .await;
    assert_eq!(names(&without.body), ["Ada", "Bob"]);
    let with = w
        .app
        .get(
            &format!("{project_path}/export?include_history=true"),
            &w.cookie,
        )
        .await;
    assert_eq!(names(&with.body), ["Ada", "Bob", "Carol"]);
}

#[tokio::test]
async fn export_needs_a_session_and_an_existing_project() {
    let w = world().await;
    let anonymous = w
        .app
        .get(&format!("/api/v1/projects/{}/export", w.project), "")
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let missing = w
        .app
        .get(
            "/api/v1/projects/00000000-0000-7000-8000-000000000000/export",
            &w.cookie,
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}
