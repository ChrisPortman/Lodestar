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
    let project = app
        .send(
            Method::POST,
            "/api/v1/projects",
            &[("cookie", &cookie)],
            Some(json!({"name": "Web", "project_code": "WEBR"})),
        )
        .await
        .body["id"]
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
    World {
        app,
        cookie,
        project,
        project_etag: added.header("etag").to_owned(),
        bob,
    }
}

impl World {
    async fn create(&self, body: Value) -> Reply {
        self.app
            .send(
                Method::POST,
                "/api/v1/actions",
                &[("cookie", &self.cookie)],
                Some(body),
            )
            .await
    }

    async fn new_action(&self, description: &str, assignee: Option<&str>) -> Reply {
        let reply = self
            .create(json!({
                "project_id": self.project,
                "description": description,
                "assignee": assignee.map(|id| json!({"id": id})),
            }))
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
        reply
    }

    async fn get(&self, path: &str) -> Reply {
        self.app.send(Method::GET, path, &[], None).await
    }

    async fn put(&self, id: &str, etag: &str, body: Value) -> Reply {
        self.app
            .send(
                Method::PUT,
                &format!("/api/v1/actions/{id}"),
                &[("cookie", &self.cookie), ("if-match", etag)],
                Some(body),
            )
            .await
    }

    async fn patch(&self, id: &str, etag: &str, closed: bool) -> Reply {
        self.app
            .send(
                Method::PATCH,
                &format!("/api/v1/actions/{id}"),
                &[
                    ("cookie", &self.cookie),
                    ("if-match", etag),
                    ("content-type", "application/merge-patch+json"),
                ],
                Some(json!({"closed": closed})),
            )
            .await
    }
}

#[tokio::test]
async fn creating_an_action_gives_it_a_reference_and_location() {
    let w = world().await;
    w.new_action("First", None).await;
    let reply = w
        .new_action("Book the room\nfor Tuesday", Some(&w.bob))
        .await;
    let id = reply.body["id"].as_str().unwrap();
    assert_eq!(reply.header("location"), format!("/api/v1/actions/{id}"));
    assert_eq!(reply.header("etag"), "\"1\"");
    assert_eq!(reply.body["reference"], "A-002");
    assert_eq!(reply.body["project_code"], "WEBR");
    assert_eq!(reply.body["description"], "Book the room\nfor Tuesday");
    assert_eq!(reply.body["assignee"]["id"], w.bob.as_str());
    assert_eq!(reply.body["assignee"]["display_name"], "Bob");
    assert_eq!(reply.body["due_date"], Value::Null);
    assert_eq!(reply.body["closed"], false);
    assert_eq!(reply.body["version"], 1);

    let fetched = w.get(&format!("/api/v1/actions/{id}")).await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.body, reply.body);
}

#[tokio::test]
async fn invalid_actions_are_rejected() {
    let w = world().await;
    let stranger = uuid::Uuid::now_v7().to_string();
    for body in [
        json!({"project_id": w.project, "description": "   "}),
        json!({"project_id": w.project, "description": "x".repeat(4001)}),
        json!({"project_id": w.project, "description": "x", "assignee": {"id": stranger}}),
        json!({"project_id": w.project, "description": "x", "due_date": "31/01/2030"}),
        json!({"project_id": w.project, "description": "x", "reference": "A-009"}),
        json!({"project_id": stranger, "description": "x"}),
        json!({"description": "x"}),
    ] {
        let reply = w.create(body.clone()).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
}

#[tokio::test]
async fn an_action_can_be_edited_closed_and_reopened() {
    let w = world().await;
    let created = w.new_action("Draft plan", None).await;
    let id = created.body["id"].as_str().unwrap();

    let mut body = created.body.clone();
    body["description"] = json!("Draft the plan");
    body["assignee"] = json!({"id": w.bob});
    body["due_date"] = json!("2001-02-03");
    let edited = w.put(id, "\"1\"", body).await;
    assert_eq!(edited.status, StatusCode::OK, "{:?}", edited.body);
    assert_eq!(edited.header("etag"), "\"2\"");
    assert_eq!(edited.body["due_date"], "2001-02-03");
    assert_eq!(edited.body["assignee"]["display_name"], "Bob");

    let closed = w.patch(id, "\"2\"", true).await;
    assert_eq!(closed.status, StatusCode::OK);
    assert_eq!(closed.body["closed"], true);
    let reopened = w.patch(id, "\"3\"", false).await;
    assert_eq!(reopened.body["closed"], false);
    assert_eq!(reopened.body["version"], 4);
}

#[tokio::test]
async fn a_stale_edit_is_412_and_a_missing_if_match_is_428() {
    let w = world().await;
    let created = w.new_action("Draft plan", None).await;
    let id = created.body["id"].as_str().unwrap();
    w.patch(id, "\"1\"", true).await;

    let stale = w.put(id, "\"1\"", created.body.clone()).await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    let stale = w.patch(id, "\"1\"", false).await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);

    let missing = w
        .app
        .send(
            Method::PUT,
            &format!("/api/v1/actions/{id}"),
            &[("cookie", &w.cookie)],
            Some(created.body.clone()),
        )
        .await;
    assert_eq!(missing.status, StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn closed_actions_are_listed_only_on_request() {
    let w = world().await;
    let first = w.new_action("One", None).await;
    w.new_action("Two", None).await;
    w.patch(first.body["id"].as_str().unwrap(), "\"1\"", true)
        .await;

    let base = format!("/api/v1/actions?project_id={}", w.project);
    let open = w.get(&base).await;
    assert_eq!(open.status, StatusCode::OK);
    assert_eq!(open.body["total"], 1);
    assert_eq!(open.body["items"][0]["reference"], "A-002");
    let legacy = w.get(&format!("{base}&include_closed=true")).await;
    assert_eq!(
        legacy.body["total"], 1,
        "only `state` selects closed Actions"
    );
    let all = w.get(&format!("{base}&state=all")).await;
    let refs: Vec<_> = all.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["reference"].as_str().unwrap())
        .collect();
    assert_eq!(refs, ["A-001", "A-002"]);
}

#[tokio::test]
async fn actions_are_never_deleted() {
    let w = world().await;
    let created = w.new_action("Draft", None).await;
    let id = created.body["id"].as_str().unwrap();
    for path in [
        "/api/v1/actions".to_owned(),
        format!("/api/v1/actions/{id}"),
    ] {
        let reply = w
            .app
            .send(Method::DELETE, &path, &[("cookie", &w.cookie)], None)
            .await;
        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    }
}

#[tokio::test]
async fn removing_a_member_unassigns_their_open_actions() {
    let w = world().await;
    let open = w.new_action("Open", Some(&w.bob)).await;
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
    let after = w.get(&format!("/api/v1/actions/{id}")).await;
    assert_eq!(after.body["assignee"], Value::Null);
    assert_eq!(after.body["version"], 2);
}

#[tokio::test]
async fn actions_in_an_archived_project_cannot_be_changed() {
    let w = world().await;
    let created = w.new_action("Draft", None).await;
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
    let reply = w
        .patch(created.body["id"].as_str().unwrap(), "\"1\"", true)
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    let reply = w
        .create(json!({"project_id": w.project, "description": "More"}))
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
}

fn references(reply: &Reply) -> Vec<String> {
    reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["reference"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn actions_can_be_filtered_by_assignee_or_unassigned() {
    let w = world().await;
    w.new_action("Bob's", Some(&w.bob)).await;
    w.new_action("Nobody's", None).await;

    let base = format!("/api/v1/actions?project_id={}", w.project);
    let bobs = w.get(&format!("{base}&assignee={}", w.bob)).await;
    assert_eq!(bobs.status, StatusCode::OK, "{:?}", bobs.body);
    assert_eq!(references(&bobs), ["A-001"]);
    assert_eq!(bobs.body["total"], 1);
    let unassigned = w.get(&format!("{base}&assignee=unassigned")).await;
    assert_eq!(references(&unassigned), ["A-002"]);
    assert_eq!(unassigned.body["total"], 1);
    let bad = w.get(&format!("{base}&assignee=nobody")).await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn actions_can_be_filtered_by_state() {
    let w = world().await;
    let first = w.new_action("One", None).await;
    w.new_action("Two", None).await;
    w.patch(first.body["id"].as_str().unwrap(), "\"1\"", true)
        .await;

    let base = format!("/api/v1/actions?project_id={}", w.project);
    assert_eq!(
        references(&w.get(&format!("{base}&state=closed")).await),
        ["A-001"]
    );
    assert_eq!(
        references(&w.get(&format!("{base}&state=open")).await),
        ["A-002"]
    );
    let all = w.get(&format!("{base}&state=all")).await;
    assert_eq!(references(&all), ["A-001", "A-002"]);
    assert_eq!(all.body["total"], 2);
}

#[tokio::test]
async fn actions_can_be_filtered_to_those_due_by_a_date() {
    let w = world().await;
    for (description, due) in [("Early", "2026-03-01"), ("Late", "2026-05-01")] {
        let reply = w
            .create(json!({
                "project_id": w.project,
                "description": description,
                "due_date": due,
            }))
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
    }
    w.new_action("Whenever", None).await;

    let base = format!("/api/v1/actions?project_id={}", w.project);
    let due = w.get(&format!("{base}&due_by=2026-03-01")).await;
    assert_eq!(due.status, StatusCode::OK, "{:?}", due.body);
    assert_eq!(references(&due), ["A-001"]);
    assert_eq!(due.body["total"], 1);
    let bad = w.get(&format!("{base}&due_by=soon")).await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn writing_needs_a_session() {
    let w = world().await;
    let reply = w
        .app
        .send(
            Method::POST,
            "/api/v1/actions",
            &[],
            Some(json!({"project_id": w.project, "description": "x"})),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

fn labels(reply: &Reply) -> Vec<String> {
    reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| format!("{}/{}", a["project_code"], a["reference"]).replace('"', ""))
        .collect()
}

#[tokio::test]
async fn actions_are_listed_by_id_unless_sorted() {
    let w = world().await;
    let late = w.new_action("Late", Some(&w.bob)).await;
    w.put(
        late.body["id"].as_str().unwrap(),
        "\"1\"",
        json!({"description": "Late", "assignee": {"id": w.bob}, "due_date": "2026-12-01"}),
    )
    .await;
    let early = w.new_action("Early", None).await;
    w.put(
        early.body["id"].as_str().unwrap(),
        "\"1\"",
        json!({"description": "Early", "assignee": null, "due_date": "2026-11-01"}),
    )
    .await;
    let apps = w
        .app
        .send(
            Method::POST,
            "/api/v1/projects",
            &[("cookie", &w.cookie)],
            Some(json!({"name": "Apps", "project_code": "APPS"})),
        )
        .await
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    w.create(json!({"project_id": apps, "description": "Undated"}))
        .await;

    assert_eq!(
        labels(&w.get(&format!("/api/v1/actions{}", "")).await),
        ["WEBR/A-001", "WEBR/A-002", "APPS/A-001"]
    );
    assert_eq!(
        labels(&w.get(&format!("/api/v1/actions{}", "?sort=id")).await),
        ["WEBR/A-001", "WEBR/A-002", "APPS/A-001"]
    );
    assert_eq!(
        labels(
            &w.get(&format!("/api/v1/actions{}", "?sort=reference"))
                .await
        ),
        ["APPS/A-001", "WEBR/A-001", "WEBR/A-002"]
    );
    assert_eq!(
        labels(&w.get(&format!("/api/v1/actions{}", "?sort=due_date")).await),
        ["WEBR/A-002", "WEBR/A-001", "APPS/A-001"],
        "earliest first, undated last"
    );
    assert_eq!(
        labels(&w.get(&format!("/api/v1/actions{}", "?sort=assignee")).await),
        ["WEBR/A-001", "WEBR/A-002", "APPS/A-001"],
        "by display name, unassigned last"
    );
    let bad = w
        .get(&format!("/api/v1/actions{}", "?sort=description"))
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
}
