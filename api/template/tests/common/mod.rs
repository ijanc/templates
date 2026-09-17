// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Start the app on an ephemeral port and talk to it over HTTP.

// Everything the tests take from the crate, so they only import from here.
#[allow(unused_imports)]
pub use {{crate_name}}::{
    AppState, OPENAPI_PATH, SWAGGER_PATH, app,
{%- if cache %}
    cache::Config as CacheConfig,
{%- endif %}
    config::RateLimit,
    error::{FieldError, Problem},
    metrics,
    model::{Item, Page},
    routes::Health,
    store::Store,
};

pub struct TestServer {
    pub base_url: String,
    pub client: reqwest::Client,
}

impl TestServer {
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
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
    let app = app(AppState { store }, rate_limit)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Some(TestServer {
        base_url: format!("http://{addr}"),
        client: reqwest::Client::new(),
    })
}
