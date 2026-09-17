// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Start the app on an ephemeral port and talk to it over HTTP.

// Everything the tests take from the crate, so they only import from here.
#[allow(unused_imports)]
pub use api_generated::{
    AppState, OPENAPI_PATH, SWAGGER_PATH, app,
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
    let store = Store::new();
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
