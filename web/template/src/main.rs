// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use {{crate_name}}::{
    AppState, PROG, app, config::Config, render::Templates, session,
    store::Store, telemetry,
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
    tracing::info!(version = %{{crate_name}}::version(), "starting");

{%- if sqlx %}
    let store = Store::connect(&cfg.database_url).await?;
    if cfg.run_migrations {
        store.migrate().await?;
        tracing::info!("migrations applied");
    }
{%- else %}
    let store = Store::new();
{%- endif %}
{%- if auth != "none" %}
    if cfg.auth.bootstrap_invite.is_some() {
        tracing::info!(
            "bootstrap invite accepted while no account exists; \
             unset it once the first user has registered"
        );
    }
{%- endif %}

    let sessions = session::store(&store).await?;
{%- if sqlx %}
    session::sweep_expired(sessions.clone());
{%- endif %}
    let session = session::layer(sessions, &cfg.session);

    let state = AppState {
        store,
        templates: Templates::new(&cfg.templates),
{%- if auth != "none" %}
        auth: cfg.auth.clone(),
{%- endif %}
    };

    let listener = tokio::net::TcpListener::bind(cfg.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    let app = app(state, session, &cfg.static_dir);
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
