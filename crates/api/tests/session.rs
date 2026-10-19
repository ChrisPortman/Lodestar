mod support;

use axum::http::{Method, StatusCode};
use serde_json::json;
use support::app;

#[tokio::test]
async fn creating_the_first_person_makes_them_the_acting_person() {
    let app = app().await;
    let created = app.first_person("Ada").await;

    let set_cookie = created.header("set-cookie").to_ascii_lowercase();
    assert!(set_cookie.starts_with("lodestar_session="), "{set_cookie}");
    assert!(set_cookie.contains("httponly"), "{set_cookie}");
    assert!(set_cookie.contains("samesite=strict"), "{set_cookie}");
    assert!(set_cookie.contains("path=/"), "{set_cookie}");
    assert!(set_cookie.contains("max-age="), "{set_cookie}");

    let session = app.get("/api/v1/session", &created.cookie()).await;
    assert_eq!(session.status, StatusCode::OK);
    assert_eq!(session.body["person"]["id"], created.body["id"]);
    assert_eq!(session.body["person"]["display_name"], "Ada");
}

#[tokio::test]
async fn an_unsigned_cookie_is_rejected() {
    let app = app().await;
    let created = app.first_person("Ada").await;
    let id = created.body["id"].as_str().unwrap();

    // A forged cookie naming the Person, with no valid signature.
    let forged = format!("lodestar_session={id}");
    assert_eq!(
        app.get("/api/v1/session", &forged).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_tampered_cookie_is_rejected() {
    let app = app().await;
    let ada = app.first_person("Ada").await;
    let bea = app.first_person("Bea").await;
    let bea_id = bea.body["id"].as_str().unwrap();
    let ada_id = ada.body["id"].as_str().unwrap();

    // Keep Ada's signature but swap in Bea's id.
    let tampered = ada.cookie().replace(ada_id, bea_id);
    assert_ne!(tampered, ada.cookie());
    assert_eq!(
        app.get("/api/v1/session", &tampered).await.status,
        StatusCode::UNAUTHORIZED
    );

    let write = app
        .send(
            Method::POST,
            "/api/v1/session",
            &[("cookie", &tampered)],
            Some(json!({"person_id": ada_id})),
        )
        .await;
    // Choosing an Acting Person needs no session, so this still works.
    assert_eq!(write.status, StatusCode::OK);

    let rename = app
        .send(
            Method::PUT,
            &format!("/api/v1/people/{ada_id}"),
            &[("cookie", &tampered), ("if-match", "\"1\"")],
            Some(json!({"display_name": "X"})),
        )
        .await;
    assert_eq!(rename.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn without_a_session_get_session_is_401() {
    let app = app().await;
    let reply = app.get("/api/v1/session", "").await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.header("content-type"), "application/problem+json");
}

#[tokio::test]
async fn post_session_switches_the_acting_person() {
    let app = app().await;
    let ada = app.first_person("Ada").await;
    let bea = app
        .send(
            Method::POST,
            "/api/v1/people",
            &[("cookie", &ada.cookie())],
            Some(json!({"display_name": "Bea"})),
        )
        .await;
    // Creating with a session keeps the existing Acting Person.
    assert_eq!(bea.header("set-cookie"), "");

    let switched = app
        .send(
            Method::POST,
            "/api/v1/session",
            &[("cookie", &ada.cookie())],
            Some(json!({"person_id": bea.body["id"]})),
        )
        .await;
    assert_eq!(switched.status, StatusCode::OK);
    assert_eq!(switched.body["person"]["display_name"], "Bea");

    let session = app.get("/api/v1/session", &switched.cookie()).await;
    assert_eq!(session.body["person"]["id"], bea.body["id"]);
}

#[tokio::test]
async fn post_session_for_an_unknown_person_is_422() {
    let app = app().await;
    let reply = app
        .send(
            Method::POST,
            "/api/v1/session",
            &[],
            Some(json!({"person_id": "0192f0c4-0000-7000-8000-000000000000"})),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.header("set-cookie"), "");
}
