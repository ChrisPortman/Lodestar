//! Drives the real router, use cases and a fresh SQLite database.

#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use lodestar_api::Services;
use lodestar_datarepo::sqlite::SqliteDatabase;
use serde_json::Value;
use tower::ServiceExt;

pub const KEY: [u8; 64] = [42; 64];

pub struct App {
    router: Router,
    _dir: tempfile::TempDir,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

impl Reply {
    pub fn header(&self, name: impl header::AsHeaderName) -> &str {
        self.headers
            .get(name)
            .map(|v| v.to_str().unwrap())
            .unwrap_or("")
    }

    /// The `name=value` part of the `Set-Cookie` header, for sending back.
    pub fn cookie(&self) -> String {
        self.header(header::SET_COOKIE)
            .split(';')
            .next()
            .unwrap()
            .to_owned()
    }
}

pub async fn app() -> App {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", dir.path().join("lodestar.db").display());
    let db = SqliteDatabase::connect(&url).await.unwrap();
    let store = db.store();
    let router = lodestar_api::router(Services {
        health: db,
        store,
        session_key: KEY.to_vec(),
    });
    App { router, _dir: dir }
}

impl App {
    pub async fn send(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<Value>,
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let body = match body {
            Some(json) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .router
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        };
        Reply {
            status,
            headers,
            body,
        }
    }

    pub async fn get(&self, path: &str, cookie: &str) -> Reply {
        self.send(Method::GET, path, &[("cookie", cookie)], None)
            .await
    }

    /// Creates a Person without a session, returning the reply (whose cookie
    /// names the new Person as Acting Person).
    pub async fn first_person(&self, name: &str) -> Reply {
        self.send(
            Method::POST,
            "/api/v1/people",
            &[],
            Some(serde_json::json!({"display_name": name})),
        )
        .await
    }
}
