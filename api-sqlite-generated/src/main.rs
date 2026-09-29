// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::net::SocketAddr;

use api_sqlite_generated::{
    AppState, PROG, app, config::Config, store::Store, telemetry,
};

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
    tracing::info!(version = %api_sqlite_generated::version(), "starting");
    let store = Store::connect(&cfg.database_url, cfg.cache.as_ref()).await?;
    if cfg.run_migrations {
        store.migrate().await?;
        tracing::info!("migrations applied");
    }

    let listener = listen(cfg.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    let app = app(AppState { store }, cfg.rate_limit)
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("stopped");
    Ok(())
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
