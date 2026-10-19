mod support;

use axum::http::{Method, StatusCode};
use serde_json::json;
use support::app;

const PROBLEM: &str = "application/problem+json";

#[tokio::test]
async fn creating_a_person_returns_201_with_location_etag_and_body() {
    let app = app().await;
    let reply = app.first_person("  Ada ").await;

    assert_eq!(reply.status, StatusCode::CREATED);
    let id = reply.body["id"].as_str().unwrap();
    assert_eq!(reply.body["display_name"], "Ada");
    assert_eq!(reply.body["version"], 1);
    assert_eq!(reply.header("location"), format!("/api/v1/people/{id}"));
    assert_eq!(reply.header("etag"), "\"1\"");
}

#[tokio::test]
async fn a_person_can_be_fetched_with_its_etag() {
    let app = app().await;
    let created = app.first_person("Ada").await;
    let id = created.body["id"].as_str().unwrap();

    let reply = app.get(&format!("/api/v1/people/{id}"), "").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, created.body);
    assert_eq!(reply.header("etag"), "\"1\"");
}

#[tokio::test]
async fn an_unknown_person_is_a_404_problem() {
    let app = app().await;
    let reply = app
        .get("/api/v1/people/0192f0c4-0000-7000-8000-000000000000", "")
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.header("content-type"), PROBLEM);
    assert_eq!(reply.body["status"], 404);
    assert!(reply.body["title"].is_string());
}

#[tokio::test]
async fn a_malformed_id_is_a_problem() {
    let app = app().await;
    let reply = app.get("/api/v1/people/not-a-uuid", "").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.header("content-type"), PROBLEM);
}

#[tokio::test]
async fn an_invalid_display_name_is_a_422_problem() {
    let app = app().await;
    for body in [
        json!({"display_name": "   "}),
        json!({"display_name": "x".repeat(101)}),
        json!({}),
        json!({"display_name": "Ada", "colour": "red"}),
    ] {
        let reply = app
            .send(Method::POST, "/api/v1/people", &[], Some(body.clone()))
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(reply.header("content-type"), PROBLEM);
        assert_eq!(reply.body["status"], 422);
    }
}

#[tokio::test]
async fn people_are_listed_in_pages_by_id_unless_sorted() {
    let app = app().await;
    let cookie = app.first_person("Charlie").await.cookie();
    for name in ["Ada", "Bea"] {
        let reply = app
            .send(
                Method::POST,
                "/api/v1/people",
                &[("cookie", &cookie)],
                Some(json!({"display_name": name})),
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
    }

    let names = |reply: &support::Reply| -> Vec<String> {
        reply.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["display_name"].as_str().unwrap().to_owned())
            .collect()
    };

    let page = app.get("/api/v1/people?limit=2&offset=1", "").await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.body["total"], 3);
    assert_eq!(page.body["limit"], 2);
    assert_eq!(page.body["offset"], 1);
    assert_eq!(names(&page), ["Ada", "Bea"]);

    let sorted = app.get("/api/v1/people?sort=display_name", "").await;
    assert_eq!(names(&sorted), ["Ada", "Bea", "Charlie"]);
    assert_eq!(sorted.body["offset"], 0);
    assert_eq!(sorted.body["limit"], 50);
}

#[tokio::test]
async fn bad_list_parameters_are_a_422_problem() {
    let app = app().await;
    for query in ["limit=0", "limit=1001", "sort=colour", "offset=-1"] {
        let reply = app.get(&format!("/api/v1/people?{query}"), "").await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(reply.header("content-type"), PROBLEM);
    }
}

#[tokio::test]
async fn deleting_a_person_is_not_allowed() {
    let app = app().await;
    let created = app.first_person("Ada").await;
    let id = created.body["id"].as_str().unwrap();
    let cookie = created.cookie();

    for path in [format!("/api/v1/people/{id}"), "/api/v1/people".to_owned()] {
        let reply = app
            .send(Method::DELETE, &path, &[("cookie", &cookie)], None)
            .await;
        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(reply.header("content-type"), PROBLEM);
        assert!(!reply.header("allow").contains("DELETE"));
    }
}

mod put {
    use super::*;

    async fn put(
        app: &support::App,
        id: &str,
        cookie: &str,
        if_match: Option<&str>,
        body: serde_json::Value,
    ) -> support::Reply {
        let mut headers = vec![("cookie", cookie)];
        if let Some(etag) = if_match {
            headers.push(("if-match", etag));
        }
        app.send(
            Method::PUT,
            &format!("/api/v1/people/{id}"),
            &headers,
            Some(body),
        )
        .await
    }

    #[tokio::test]
    async fn renames_with_a_matching_if_match() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let id = created.body["id"].as_str().unwrap();

        let reply = put(
            &app,
            id,
            &created.cookie(),
            Some("\"1\""),
            json!({"display_name": "Ada King"}),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.body["display_name"], "Ada King");
        assert_eq!(reply.body["version"], 2);
        assert_eq!(reply.header("etag"), "\"2\"");
    }

    #[tokio::test]
    async fn accepts_matching_read_only_fields() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let id = created.body["id"].as_str().unwrap();

        let reply = put(
            &app,
            id,
            &created.cookie(),
            Some("\"1\""),
            json!({"id": id, "version": 1, "display_name": "Ada King"}),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn without_if_match_is_428() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let id = created.body["id"].as_str().unwrap();

        let reply = put(
            &app,
            id,
            &created.cookie(),
            None,
            json!({"display_name": "Ada King"}),
        )
        .await;
        assert_eq!(reply.status, StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(reply.header("content-type"), PROBLEM);
    }

    #[tokio::test]
    async fn with_a_stale_if_match_or_version_is_412() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let id = created.body["id"].as_str().unwrap();
        let cookie = created.cookie();
        put(
            &app,
            id,
            &cookie,
            Some("\"1\""),
            json!({"display_name": "Ada King"}),
        )
        .await;

        let stale_header = put(
            &app,
            id,
            &cookie,
            Some("\"1\""),
            json!({"display_name": "Ada L"}),
        )
        .await;
        assert_eq!(stale_header.status, StatusCode::PRECONDITION_FAILED);
        assert_eq!(stale_header.header("content-type"), PROBLEM);

        let stale_body = put(
            &app,
            id,
            &cookie,
            Some("\"2\""),
            json!({"version": 1, "display_name": "Ada L"}),
        )
        .await;
        assert_eq!(stale_body.status, StatusCode::PRECONDITION_FAILED);

        let now = app.get(&format!("/api/v1/people/{id}"), "").await;
        assert_eq!(now.body["display_name"], "Ada King");
    }

    #[tokio::test]
    async fn with_a_mismatched_id_or_missing_field_is_422() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let id = created.body["id"].as_str().unwrap();
        let other = app
            .send(
                Method::POST,
                "/api/v1/people",
                &[("cookie", &created.cookie())],
                Some(json!({"display_name": "Bea"})),
            )
            .await;
        let other_id = other.body["id"].as_str().unwrap();

        for body in [
            json!({"id": other_id, "display_name": "Ada King"}),
            json!({}),
            json!({"display_name": ""}),
        ] {
            let reply = put(&app, id, &created.cookie(), Some("\"1\""), body.clone()).await;
            assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        }
    }

    #[tokio::test]
    async fn an_unknown_person_is_404() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let reply = put(
            &app,
            "0192f0c4-0000-7000-8000-000000000000",
            &created.cookie(),
            Some("\"1\""),
            json!({"display_name": "X"}),
        )
        .await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn without_a_session_is_401() {
        let app = app().await;
        let created = app.first_person("Ada").await;
        let id = created.body["id"].as_str().unwrap();

        let reply = put(&app, id, "", Some("\"1\""), json!({"display_name": "X"})).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        assert_eq!(reply.header("content-type"), PROBLEM);
    }
}
