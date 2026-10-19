mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

const PROBLEM: &str = "application/problem+json";

/// An app with one Person, their session cookie, and a Project's ID.
async fn with_project() -> (App, String, String) {
    let app = app().await;
    let cookie = app.first_person("Ada").await.cookie();
    let project = app
        .send(
            Method::POST,
            "/api/v1/projects",
            &[("cookie", &cookie)],
            Some(json!({"name": "Website", "project_code": "WEBR"})),
        )
        .await;
    assert_eq!(project.status, StatusCode::CREATED);
    let id = project.body["id"].as_str().unwrap().to_owned();
    (app, cookie, id)
}

async fn create(app: &App, cookie: &str, body: Value) -> Reply {
    app.send(
        Method::POST,
        "/api/v1/risks",
        &[("cookie", cookie)],
        Some(body),
    )
    .await
}

async fn new_risk(app: &App, cookie: &str, project: &str, desc: &str, l: u8, i: u8) -> Reply {
    let reply = create(
        app,
        cookie,
        json!({"project_id": project, "description": desc, "likelihood": l, "impact": i}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    reply
}

async fn put(app: &App, cookie: &str, id: &str, etag: &str, body: Value) -> Reply {
    app.send(
        Method::PUT,
        &format!("/api/v1/risks/{id}"),
        &[("cookie", cookie), ("if-match", etag)],
        Some(body),
    )
    .await
}

async fn patch(app: &App, cookie: &str, id: &str, etag: &str, body: Value) -> Reply {
    app.send(
        Method::PATCH,
        &format!("/api/v1/risks/{id}"),
        &[
            ("cookie", cookie),
            ("if-match", etag),
            ("content-type", "application/merge-patch+json"),
        ],
        Some(body),
    )
    .await
}

fn descriptions(reply: &Reply) -> Vec<String> {
    reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["description"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn creating_a_risk_gives_it_a_reference_and_score() {
    let (app, cookie, project) = with_project().await;
    let reply = new_risk(&app, &cookie, &project, " Vendor late ", 3, 4).await;

    let id = reply.body["id"].as_str().unwrap();
    assert_eq!(reply.header("location"), format!("/api/v1/risks/{id}"));
    assert_eq!(reply.header("etag"), "\"1\"");
    assert_eq!(reply.body["project_id"], project.as_str());
    assert_eq!(reply.body["project_code"], "WEBR");
    assert_eq!(reply.body["reference"], "R-001");
    assert_eq!(reply.body["description"], "Vendor late");
    assert_eq!(reply.body["likelihood"], 3);
    assert_eq!(reply.body["impact"], 4);
    assert_eq!(reply.body["score"], 12);
    assert_eq!(reply.body["closed"], false);
    assert_eq!(reply.body["version"], 1);

    let fetched = app.get(&format!("/api/v1/risks/{id}"), "").await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.body, reply.body);
    let second = new_risk(&app, &cookie, &project, "Another", 1, 1).await;
    assert_eq!(second.body["reference"], "R-002");
}

#[tokio::test]
async fn creating_needs_a_session() {
    let (app, _, project) = with_project().await;
    let reply = create(
        &app,
        "",
        json!({"project_id": project, "description": "Risk", "likelihood": 1, "impact": 1}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn ratings_out_of_range_and_other_invalid_risks_are_a_422_problem() {
    let (app, cookie, project) = with_project().await;
    for body in [
        json!({"project_id": project, "description": "Risk", "likelihood": 0, "impact": 3}),
        json!({"project_id": project, "description": "Risk", "likelihood": 6, "impact": 3}),
        json!({"project_id": project, "description": "Risk", "likelihood": 3, "impact": 0}),
        json!({"project_id": project, "description": "Risk", "likelihood": 3, "impact": 6}),
        json!({"project_id": project, "description": "Risk", "likelihood": -1, "impact": 3}),
        json!({"project_id": project, "description": " ", "likelihood": 3, "impact": 3}),
        json!({"project_id": project, "description": "Risk", "likelihood": 3}),
        json!({"description": "Risk", "likelihood": 3, "impact": 3}),
        json!({"project_id": project, "description": "Risk", "likelihood": 3, "impact": 3, "closed": true}),
        json!({"project_id": project, "description": "Risk", "likelihood": 3, "impact": 3, "score": 9}),
        json!({"project_id": "00000000-0000-7000-8000-000000000000", "description": "Risk", "likelihood": 3, "impact": 3}),
    ] {
        let reply = create(&app, &cookie, body.clone()).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(reply.header("content-type"), PROBLEM);
    }
}

#[tokio::test]
async fn editing_replaces_description_likelihood_and_impact() {
    let (app, cookie, project) = with_project().await;
    let risk = new_risk(&app, &cookie, &project, "Risk", 1, 1).await;
    let id = risk.body["id"].as_str().unwrap();

    let mut body = risk.body.clone();
    body["description"] = json!("Bigger risk");
    body["likelihood"] = json!(5);
    body["impact"] = json!(4);
    let reply = put(&app, &cookie, id, "\"1\"", body).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.header("etag"), "\"2\"");
    assert_eq!(reply.body["score"], 20);
    assert_eq!(reply.body["description"], "Bigger risk");

    let stale = put(
        &app,
        &cookie,
        id,
        "\"1\"",
        json!({"description": "Risk", "likelihood": 1, "impact": 1}),
    )
    .await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    let out_of_range = put(
        &app,
        &cookie,
        id,
        "\"2\"",
        json!({"description": "Risk", "likelihood": 1, "impact": 7}),
    )
    .await;
    assert_eq!(out_of_range.status, StatusCode::UNPROCESSABLE_ENTITY);
    let mismatched = put(
        &app,
        &cookie,
        id,
        "\"2\"",
        json!({"description": "Risk", "likelihood": 1, "impact": 1, "reference": "R-009"}),
    )
    .await;
    assert_eq!(mismatched.status, StatusCode::UNPROCESSABLE_ENTITY);
    let missing = app
        .send(
            Method::PUT,
            &format!("/api/v1/risks/{id}"),
            &[("cookie", &cookie)],
            Some(json!({"description": "Risk", "likelihood": 1, "impact": 1})),
        )
        .await;
    assert_eq!(missing.status, StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn closing_and_reopening_uses_merge_patch() {
    let (app, cookie, project) = with_project().await;
    let risk = new_risk(&app, &cookie, &project, "Risk", 1, 1).await;
    let id = risk.body["id"].as_str().unwrap();

    let closed = patch(&app, &cookie, id, "\"1\"", json!({"closed": true})).await;
    assert_eq!(closed.status, StatusCode::OK);
    assert_eq!(closed.body["closed"], true);
    let listed = app
        .get(&format!("/api/v1/risks?project_id={project}"), "")
        .await;
    assert_eq!(listed.body["total"], 0);
    let legacy = app
        .get(
            &format!("/api/v1/risks?project_id={project}&include_closed=true"),
            "",
        )
        .await;
    assert_eq!(legacy.body["total"], 0, "only `state` selects closed Risks");
    let with_closed = app
        .get(&format!("/api/v1/risks?project_id={project}&state=all"), "")
        .await;
    assert_eq!(with_closed.body["total"], 1);

    let reopened = patch(&app, &cookie, id, "\"2\"", json!({"closed": false})).await;
    assert_eq!(reopened.body["closed"], false);
    assert_eq!(reopened.body["version"], 3);
    let invalid = patch(&app, &cookie, id, "\"3\"", json!({"likelihood": 2})).await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn risks_can_be_sorted_by_score() {
    let (app, cookie, project) = with_project().await;
    for (d, l, i) in [("Low", 1, 2), ("High", 5, 5), ("Mid", 3, 3)] {
        new_risk(&app, &cookie, &project, d, l, i).await;
    }
    let path = format!("/api/v1/risks?project_id={project}");
    let by_id = app.get(&path, "").await;
    assert_eq!(descriptions(&by_id), ["Low", "High", "Mid"]);
    let by_score = app.get(&format!("{path}&sort=score"), "").await;
    assert_eq!(descriptions(&by_score), ["High", "Mid", "Low"]);
    assert_eq!(by_score.body["items"][0]["score"], 25);
    let by_reference = app
        .get(&format!("{path}&sort=reference&limit=1&offset=2"), "")
        .await;
    assert_eq!(descriptions(&by_reference), ["Mid"]);
    assert_eq!(by_reference.body["total"], 3);
    let bad = app.get(&format!("{path}&sort=size"), "").await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn risks_can_be_filtered_by_state() {
    let (app, cookie, project) = with_project().await;
    let first = new_risk(&app, &cookie, &project, "Gone", 1, 1).await;
    new_risk(&app, &cookie, &project, "Live", 1, 1).await;
    let id = first.body["id"].as_str().unwrap();
    patch(&app, &cookie, id, "\"1\"", json!({"closed": true})).await;
    let path = format!("/api/v1/risks?project_id={project}");
    let closed = app.get(&format!("{path}&state=closed"), "").await;
    assert_eq!(descriptions(&closed), ["Gone"]);
    assert_eq!(closed.body["total"], 1);
    let all = app.get(&format!("{path}&state=all"), "").await;
    assert_eq!(descriptions(&all), ["Gone", "Live"]);
}

#[tokio::test]
async fn risks_in_an_archived_project_are_read_only() {
    let (app, cookie, project) = with_project().await;
    let risk = new_risk(&app, &cookie, &project, "Risk", 1, 1).await;
    let id = risk.body["id"].as_str().unwrap();
    let archived = app
        .send(
            Method::PATCH,
            &format!("/api/v1/projects/{project}"),
            &[
                ("cookie", &cookie),
                ("if-match", "\"1\""),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(json!({"archived": true})),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK);

    let created = create(
        &app,
        &cookie,
        json!({"project_id": project, "description": "Risk", "likelihood": 1, "impact": 1}),
    )
    .await;
    assert_eq!(created.status, StatusCode::CONFLICT);
    let closed = patch(&app, &cookie, id, "\"1\"", json!({"closed": true})).await;
    assert_eq!(closed.status, StatusCode::CONFLICT);
    assert_eq!(closed.header("content-type"), PROBLEM);
}

#[tokio::test]
async fn risks_are_never_deleted() {
    let (app, cookie, project) = with_project().await;
    let risk = new_risk(&app, &cookie, &project, "Risk", 1, 1).await;
    let id = risk.body["id"].as_str().unwrap();
    let reply = app
        .send(
            Method::DELETE,
            &format!("/api/v1/risks/{id}"),
            &[("cookie", &cookie)],
            None,
        )
        .await;
    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        app.get("/api/v1/risks/not-a-uuid", "").await.status,
        StatusCode::BAD_REQUEST
    );
}
