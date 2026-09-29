// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Composition root: read the configuration, build the store, the use
//! cases over it and the HTTP adapter over those, then serve.

mod config;
mod telemetry;

use std::{net::SocketAddr, sync::Arc};

use application::ItemService;
use http_adapter::{AppState, app};
{%- if sqlx %}
use store_sqlx::SqlxStore;
{%- else %}
use store_memory::MemoryStore;
{%- endif %}

use crate::config::Config;

/// Program name, used in diagnostics.
pub const PROG: &str = "{{project-name}}";

fn main() {
    if let Err(e) = run() {
        eprintln!("{PROG}: {e:#}");
        std::process::exit(1);
    }
}

#[tokio::main]
async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env()?;
    telemetry::init(&cfg)?;
    tracing::info!(version = %version(), "starting");

{%- if sqlx %}
    let store = SqlxStore::connect(&cfg.database_url).await?;
    if cfg.run_migrations {
        store.migrate().await?;
        tracing::info!("migrations applied");
    }
{%- else %}
    let store = MemoryStore::new();
{%- endif %}
    let items = ItemService::new(Arc::new(store));
    let state = AppState {
        items: Arc::new(items),
        version: version(),
    };

    let listener = listen(cfg.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    let app = app(state, cfg.rate_limit)
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("stopped");
    Ok(())
}

/// Package version followed by the git hash and build date recorded by
/// `build.rs`, when available: `0.1.0 (abc1234 2026-01-31)`.
pub fn version() -> String {
    let mut v = env!("CARGO_PKG_VERSION").to_string();
    let hash = env!("{{env_prefix}}_GIT_HASH");
    let date = env!("{{env_prefix}}_BUILD_DATE");
    let extra: Vec<&str> =
        [hash, date].into_iter().filter(|s| !s.is_empty()).collect();
    if !extra.is_empty() {
        v.push_str(&format!(" ({})", extra.join(" ")));
    }
    v
}

/// Takes the socket passed in by systemfd or systemd socket activation,
/// so a restart keeps the port and queued connections; binds `addr`
/// when there is none.
async fn listen(
    addr: std::net::SocketAddr,
) -> anyhow::Result<tokio::net::TcpListener> {
    if let Some(l) = listenfd::ListenFd::from_env().take_tcp_listener(0)? {
        l.set_nonblocking(true)?;
        return Ok(tokio::net::TcpListener::from_std(l)?);
    }
    Ok(tokio::net::TcpListener::bind(addr).await?)
}

/// Resolves on SIGINT or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install SIGINT handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )
        .expect("install SIGTERM handler")
        .recv()
        .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("shutting down");
}
