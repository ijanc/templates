// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::net::SocketAddr;

use api_postgres_generated::{
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
    tracing::info!(version = %api_postgres_generated::version(), "starting");
    let store = Store::connect(&cfg.database_url).await?;
    if cfg.run_migrations {
        store.migrate().await?;
        tracing::info!("migrations applied");
    }

    let listener = tokio::net::TcpListener::bind(cfg.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    let app = app(AppState { store }, cfg.rate_limit)
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("stopped");
    Ok(())
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
