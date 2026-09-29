// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Start the app on an ephemeral port and talk to it over HTTP.

// Each test binary uses a different subset of the helpers.
#![allow(dead_code)]

// Everything the tests take from the crate, so they only import from here.
#[allow(unused_imports)]
pub use {{crate_name}}::{
    AppState, GRAPHQL_PATH, SDL_PATH, app,
{%- if cache %}
    cache::Config as CacheConfig,
{%- endif %}
    config::RateLimit,
    error::Problem,
    graphql::error::FieldError,
    metrics,
    model::{Item, Page},
    routes::Health,
    store::Store,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub struct TestServer {
    pub base_url: String,
    pub client: reqwest::Client,
}

impl TestServer {
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// Send `query` with `variables` to the GraphQL endpoint.
    pub async fn post_gql(
        &self,
        query: &str,
        variables: Value,
    ) -> reqwest::Response {
        self.client
            .post(self.url(GRAPHQL_PATH))
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .unwrap()
    }

    /// [`post_gql`](Self::post_gql), returning the whole body; the
    /// status must be 200.
    pub async fn gql(&self, query: &str, variables: Value) -> Value {
        let res = self.post_gql(query, variables).await;
        assert_eq!(res.status(), reqwest::StatusCode::OK);
        res.json().await.unwrap()
    }
}

/// `data.<field>` of a response without errors.
pub fn data<T: DeserializeOwned>(res: &Value, field: &str) -> T {
    assert!(res.get("errors").is_none(), "{res}");
    serde_json::from_value(res["data"][field].clone()).unwrap()
}

/// The only error of a response, checked to carry `code` and a
/// request id.
pub fn error<'a>(res: &'a Value, code: &str) -> &'a Value {
    let errors = res["errors"].as_array().unwrap_or_else(|| panic!("{res}"));
    assert_eq!(errors.len(), 1, "{res}");
    let e = &errors[0];
    assert_eq!(e["extensions"]["code"], code, "{res}");
    assert!(e["extensions"]["request_id"].is_string(), "{res}");
    assert!(e["message"].is_string(), "{res}");
    e
}

/// Spawn a server with a fresh store and the default rate limit.
/// Returns `None` when the backing store is unavailable, so the test is
/// skipped.
pub async fn spawn() -> Option<TestServer> {
    spawn_with(Some(RateLimit::default())).await
}

/// [`spawn`] with an explicit rate limit; `None` disables it.
pub async fn spawn_with(rate_limit: Option<RateLimit>) -> Option<TestServer> {
{%- if store == "postgres" %}
    let Some(url) =
        std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty())
    else {
        eprintln!("DATABASE_URL not set, skipping");
        return None;
    };
{%- if cache %}
    let cache = CacheConfig::default();
    let store = Store::connect(&url, Some(&cache)).await.expect("connect");
{%- else %}
    let store = Store::connect(&url).await.expect("connect");
{%- endif %}
    store.migrate().await.expect("migrate");
{%- elsif store == "sqlite" %}
{%- if cache %}
    let cache = CacheConfig::default();
    let store = Store::connect("sqlite::memory:", Some(&cache))
        .await
        .expect("connect");
{%- else %}
    let store = Store::connect("sqlite::memory:").await.expect("connect");
{%- endif %}
    store.migrate().await.expect("migrate");
{%- else %}
    let store = Store::new();
{%- endif %}
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = app(AppState::new(store), rate_limit)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Some(TestServer {
        base_url: format!("http://{addr}"),
        client: reqwest::Client::new(),
    })
}
