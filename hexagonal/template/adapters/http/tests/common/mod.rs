// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Start the app on an ephemeral port, over the in-memory store, and
//! talk to it over HTTP.

use std::sync::Arc;

use application::ItemService;
// Everything the tests take from the crate, so they only import from here.
#[allow(unused_imports)]
pub use http_adapter::{
    AppState, OPENAPI_PATH, RateLimit, SWAGGER_PATH, app,
    dto::{Item, Page},
    error::{FieldError, Problem},
    metrics,
    routes::Health,
};
use store_memory::MemoryStore;

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
pub async fn spawn() -> TestServer {
    spawn_with(Some(RateLimit::default())).await
}

/// [`spawn`] with an explicit rate limit; `None` disables it.
pub async fn spawn_with(rate_limit: Option<RateLimit>) -> TestServer {
    let items = ItemService::new(Arc::new(MemoryStore::new()));
    let state = AppState {
        items: Arc::new(items),
        version: env!("CARGO_PKG_VERSION").into(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = app(state, rate_limit)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestServer {
        base_url: format!("http://{addr}"),
        client: reqwest::Client::new(),
    }
}
