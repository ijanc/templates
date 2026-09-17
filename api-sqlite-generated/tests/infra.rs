// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Probes, request ids, metrics, API docs and rate limiting.

mod common;

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::{
    Health, OPENAPI_PATH, Problem, RateLimit, SWAGGER_PATH, metrics,
};

#[tokio::test]
async fn healthz() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url("/healthz")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let h: Health = res.json().await.unwrap();
    assert_eq!(h.status, "ok");
    assert!(
        h.version.starts_with(env!("CARGO_PKG_VERSION")),
        "{}",
        h.version
    );
}

#[tokio::test]
async fn readyz() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url("/readyz")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let h: Health = res.json().await.unwrap();
    assert_eq!(h.status, "ok");
}

#[tokio::test]
async fn request_id_generated() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url("/healthz")).send().await.unwrap();
    let id = res.headers()["x-request-id"].to_str().unwrap();
    let id = Uuid::parse_str(id).unwrap();
    assert_eq!(id.get_version(), Some(uuid::Version::Random));
}

#[tokio::test]
async fn request_id_echoed() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .get(s.url("/healthz"))
        .header("x-request-id", "abc-123")
        .send()
        .await
        .unwrap();
    assert_eq!(res.headers()["x-request-id"], "abc-123");
}

#[tokio::test]
async fn metrics_exported() {
    let Some(s) = common::spawn().await else {
        return;
    };
    s.client.get(s.url("/healthz")).send().await.unwrap();
    let res = s.client.get(s.url(metrics::PATH)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = res.text().await.unwrap();
    let counter = format!("{}_http_requests_total", metrics::PREFIX);
    assert!(text.contains(&counter), "{text}");
    assert!(text.contains("endpoint=\"/healthz\""), "{text}");
}

#[tokio::test]
async fn cache_metrics() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .post(s.url("/v1/items"))
        .json(&json!({ "name": "cached" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let url = res.headers()["location"].to_str().unwrap().to_owned();
    for _ in 0..2 {
        let res = s.client.get(s.url(&url)).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
    let res = s.client.get(s.url(metrics::PATH)).send().await.unwrap();
    let text = res.text().await.unwrap();
    let hits = format!("{}_cache_hits_total", metrics::PREFIX);
    assert!(text.contains(&hits), "{text}");
}

#[tokio::test]
async fn openapi_document() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url(OPENAPI_PATH)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let doc: Value = res.json().await.unwrap();
    assert!(doc["openapi"].as_str().unwrap().starts_with("3."));
    let paths = doc["paths"].as_object().unwrap();
    for p in ["/healthz", "/readyz", "/v1/items", "/v1/items/{id}"] {
        assert!(paths.contains_key(p), "missing {p}: {:?}", paths.keys());
    }
    assert!(doc["components"]["schemas"]["Problem"].is_object());
    assert!(doc["components"]["schemas"]["FieldError"].is_object());
}

#[tokio::test]
async fn swagger_ui() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url(SWAGGER_PATH)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = res.text().await.unwrap();
    assert!(text.contains("swagger-ui"), "{text}");
}

#[tokio::test]
async fn method_not_allowed_is_json() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.patch(s.url("/v1/items")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let body: Problem = res.json().await.unwrap();
    assert_eq!(body.status, 405);
    assert_eq!(body.code, "method_not_allowed");
    assert_eq!(body.instance.as_deref(), Some("/v1/items"));
    assert!(body.request_id.is_some());
}

#[tokio::test]
async fn rate_limited() {
    let rl = RateLimit {
        per_second: 1,
        burst: 2,
        trust_proxy: false,
    };
    let Some(s) = common::spawn_with(Some(rl)).await else {
        return;
    };
    for remaining in ["1", "0"] {
        let res = s.client.get(s.url("/v1/items")).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["x-ratelimit-limit"], "2");
        assert_eq!(res.headers()["x-ratelimit-remaining"], remaining);
    }
    let res = s.client.get(s.url("/v1/items")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(res.headers().contains_key("retry-after"), "{res:?}");
    assert_eq!(res.headers()["x-ratelimit-remaining"], "0");
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let body: Problem = res.json().await.unwrap();
    assert_eq!(body.code, "too_many_requests");
    assert_eq!(body.status, 429);
    assert!(body.request_id.is_some());

    // Probes are not limited.
    let res = s.client.get(s.url("/healthz")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(!res.headers().contains_key("x-ratelimit-limit"));
}

#[tokio::test]
async fn rate_limit_off() {
    let Some(s) = common::spawn_with(None).await else {
        return;
    };
    for _ in 0..3 {
        let res = s.client.get(s.url("/v1/items")).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!res.headers().contains_key("x-ratelimit-limit"));
    }
}
