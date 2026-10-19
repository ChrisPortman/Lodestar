mod support;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use support::{App, Reply, app};

/// Ada's session, Ada's Project (which she leads) and Bob, who is not a Member.
struct World {
    app: App,
    cookie: String,
    project: String,
    ada: String,
    bob: String,
}

async fn world() -> World {
    let app = app().await;
    let first = app.first_person("Ada").await;
    let cookie = first.cookie();
    let ada = first.body["id"].as_str().unwrap().to_owned();
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
    World {
        app,
        cookie,
        project,
        ada,
        bob,
    }
}

impl World {
    async fn add(&self, etag: &str, person: &str) -> Reply {
        self.app
            .send(
                Method::POST,
                &format!("/api/v1/projects/{}/members", self.project),
                &[("cookie", &self.cookie), ("if-match", etag)],
                Some(json!({"person_id": person})),
            )
            .await
    }

    async fn remove(&self, etag: &str, person: &str) -> Reply {
        self.app
            .send(
                Method::DELETE,
                &format!("/api/v1/projects/{}/members/{person}", self.project),
                &[("cookie", &self.cookie), ("if-match", etag)],
                None,
            )
            .await
    }

    async fn put_leader(&self, etag: &str, leader: Value) -> Reply {
        self.app
            .send(
                Method::PUT,
                &format!("/api/v1/projects/{}", self.project),
                &[("cookie", &self.cookie), ("if-match", etag)],
                Some(json!({"name": "Web", "description": null, "health_rating": null, "project_leader": leader})),
            )
            .await
    }

    async fn member_names(&self) -> Vec<String> {
        let reply = self
            .app
            .get(&format!("/api/v1/projects/{}/members", self.project), "")
            .await;
        assert_eq!(reply.status, StatusCode::OK);
        reply.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["display_name"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[tokio::test]
async fn a_person_can_be_added_and_removed_as_a_member() {
    let w = world().await;
    assert_eq!(w.member_names().await, ["Ada"]);

    let added = w.add("\"1\"", &w.bob).await;
    assert_eq!(added.status, StatusCode::OK);
    assert_eq!(added.header("etag"), "\"2\"");
    assert_eq!(added.body["version"], 2);
    assert_eq!(w.member_names().await, ["Ada", "Bob"]);

    let removed = w.remove("\"2\"", &w.bob).await;
    assert_eq!(removed.status, StatusCode::OK);
    assert_eq!(removed.header("etag"), "\"3\"");
    assert_eq!(w.member_names().await, ["Ada"]);
}

#[tokio::test]
async fn member_changes_need_a_session_and_the_current_version() {
    let w = world().await;
    let no_session = w
        .app
        .send(
            Method::POST,
            &format!("/api/v1/projects/{}/members", w.project),
            &[("if-match", "\"1\"")],
            Some(json!({"person_id": w.bob})),
        )
        .await;
    assert_eq!(no_session.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        w.add("\"9\"", &w.bob).await.status,
        StatusCode::PRECONDITION_FAILED
    );
    assert_eq!(
        w.remove("\"9\"", &w.ada).await.status,
        StatusCode::PRECONDITION_FAILED
    );
}

#[tokio::test]
async fn unknown_people_and_non_members_are_problems() {
    let w = world().await;
    let unknown = uuid_string();
    assert_eq!(
        w.add("\"1\"", &unknown).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        w.remove("\"1\"", &w.bob).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn the_leader_can_be_changed_to_another_member() {
    let w = world().await;
    w.add("\"1\"", &w.bob).await;
    let reply = w.put_leader("\"2\"", json!({"id": w.bob})).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["project_leader"]["display_name"], "Bob");
}

#[tokio::test]
async fn only_a_member_can_lead() {
    let w = world().await;
    let reply = w.put_leader("\"1\"", json!({"id": w.bob})).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn the_leader_is_a_required_put_field() {
    let w = world().await;
    let reply = w
        .app
        .send(
            Method::PUT,
            &format!("/api/v1/projects/{}", w.project),
            &[("cookie", &w.cookie), ("if-match", "\"1\"")],
            Some(json!({"name": "Web", "description": null, "health_rating": null})),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn removing_the_leader_leaves_no_leader() {
    let w = world().await;
    let reply = w.remove("\"1\"", &w.ada).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["project_leader"], Value::Null);
    assert!(w.member_names().await.is_empty());
}

#[tokio::test]
async fn an_archived_project_rejects_member_changes() {
    let w = world().await;
    let archived = w
        .app
        .send(
            Method::PATCH,
            &format!("/api/v1/projects/{}", w.project),
            &[
                ("cookie", &w.cookie),
                ("if-match", "\"1\""),
                ("content-type", "application/merge-patch+json"),
            ],
            Some(json!({"archived": true})),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK);
    assert_eq!(w.add("\"2\"", &w.bob).await.status, StatusCode::CONFLICT);
    assert_eq!(w.remove("\"2\"", &w.ada).await.status, StatusCode::CONFLICT);
}

fn uuid_string() -> String {
    "0192f3c4-0000-7000-8000-000000000000".to_owned()
}
