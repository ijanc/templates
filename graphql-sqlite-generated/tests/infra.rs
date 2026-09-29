// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Probes, request ids, metrics, GraphiQL, SDL and rate limiting.

mod common;

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    GRAPHQL_PATH, Health, Problem, RateLimit, SDL_PATH, metrics,
};

const LIST_Q: &str = "{ items { total } }";

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
    let q = "mutation { createItem(input: { name: \"cached\" }) { id } }";
    let res = s.gql(q, json!({})).await;
    let id = res["data"]["createItem"]["id"].clone();
    assert!(id.is_string(), "{res}");
    let q = "query($id: UUID!) { item(id: $id) { id } }";
    for _ in 0..2 {
        let res = s.gql(q, json!({ "id": id })).await;
        assert_eq!(res["data"]["item"]["id"], id, "{res}");
    }
    let res = s.client.get(s.url(metrics::PATH)).send().await.unwrap();
    let text = res.text().await.unwrap();
    let hits = format!("{}_cache_hits_total", metrics::PREFIX);
    assert!(text.contains(&hits), "{text}");
}

#[tokio::test]
async fn graphiql() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url(GRAPHQL_PATH)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = res.text().await.unwrap();
    assert!(text.to_lowercase().contains("graphiql"), "{text}");
}

#[tokio::test]
async fn sdl() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url(SDL_PATH)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = res.text().await.unwrap();
    for t in [
        "type Item ",
        "type ItemPage ",
        "input CreateItemInput ",
        "createItem(",
        "items(",
    ] {
        assert!(text.contains(t), "missing {t}: {text}");
    }
}

#[tokio::test]
async fn method_not_allowed_is_json() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.put(s.url(GRAPHQL_PATH)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let body: Problem = res.json().await.unwrap();
    assert_eq!(body.status, 405);
    assert_eq!(body.code, "method_not_allowed");
    assert_eq!(body.instance.as_deref(), Some(GRAPHQL_PATH));
    assert!(body.request_id.is_some());
}

#[tokio::test]
async fn unknown_route_is_problem() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url("/nothing")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let body: Problem = res.json().await.unwrap();
    assert_eq!(body.code, "not_found");
    assert_eq!(body.title, "Not Found");
    assert_eq!(body.instance.as_deref(), Some("/nothing"));
    assert!(body.r#type.ends_with(":error:not_found"), "{body:?}");
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
        let res = s.post_gql(LIST_Q, json!({})).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["x-ratelimit-limit"], "2");
        assert_eq!(res.headers()["x-ratelimit-remaining"], remaining);
    }
    let res = s.post_gql(LIST_Q, json!({})).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(res.headers().contains_key("retry-after"), "{res:?}");
    assert_eq!(res.headers()["x-ratelimit-remaining"], "0");
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let body: Problem = res.json().await.unwrap();
    assert_eq!(body.code, "too_many_requests");
    assert_eq!(body.status, 429);
    assert!(body.request_id.is_some());

    // Probes and GraphiQL are not limited.
    for path in ["/healthz", GRAPHQL_PATH] {
        let res = s.client.get(s.url(path)).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!res.headers().contains_key("x-ratelimit-limit"));
    }
}

#[tokio::test]
async fn rate_limit_off() {
    let Some(s) = common::spawn_with(None).await else {
        return;
    };
    for _ in 0..3 {
        let res = s.post_gql(LIST_Q, json!({})).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!res.headers().contains_key("x-ratelimit-limit"));
    }
}
