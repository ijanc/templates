// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::{path::Path, time::Duration};

use axum::{
    Router,
    extract::OriginalUri,
    http::{Extensions, StatusCode, Uri},
    middleware,
};
use tower_http::{
    services::ServeDir, timeout::TimeoutLayer, trace::TraceLayer,
};

pub mod auth;
pub mod config;
pub mod csrf;
pub mod error;
pub mod extract;
pub mod flash;
pub mod model;
pub mod render;
pub mod request_id;
pub mod routes;
pub mod session;
pub mod store;
pub mod telemetry;

/// Program name, used in diagnostics and as the site title.
pub const PROG: &str = "web-generated";

/// Where static files are served from.
pub const STATIC_PATH: &str = "/static";

/// Requests taking longer than this get a 408.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Largest accepted form body.
pub const FORM_LIMIT: usize = 256 * 1024;

/// Whether accounts exist at all; templates hide the nav entries and
/// the buttons that need one when this is false.
pub const AUTH: bool = true;

/// Whether the login page offers Google.
pub const AUTH_GOOGLE: bool = false;

/// Default lifetime of an invite: one week.
pub const INVITE_TTL: Duration = Duration::from_secs(604_800);

/// Shared state handed to every handler.
#[derive(Clone, Debug)]
pub struct AppState {
    pub store: store::Store,
    pub templates: render::Templates,
    pub auth: auth::Config,
}

/// Package version followed by the git hash and build date recorded by
/// `build.rs`, when available: `0.1.0 (abc1234 2026-01-31)`.
pub fn version() -> String {
    let mut v = env!("CARGO_PKG_VERSION").to_string();
    let hash = env!("WEB_GENERATED_GIT_HASH");
    let date = env!("WEB_GENERATED_BUILD_DATE");
    let extra: Vec<&str> =
        [hash, date].into_iter().filter(|s| !s.is_empty()).collect();
    if !extra.is_empty() {
        v.push_str(&format!(" ({})", extra.join(" ")));
    }
    v
}

/// The complete router: pages, static files and the middleware stack.
/// `session` comes from [`session::layer`], `static_dir` from the
/// configuration.
pub fn app(
    state: AppState,
    session: session::Layer,
    static_dir: &Path,
) -> Router {
    routes::router(&state, static_dir)
        .fallback(error::not_found)
        // Innermost first: the timeout's 408 and the token check's 403
        // both reach the page middleware, sessions are read before the
        // token check needs them, and the request id is set last, so it
        // exists before anything outside looks at it.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(middleware::from_fn(csrf::verify))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            error::page_middleware,
        ))
        .layer(session)
        .layer(TraceLayer::new_for_http().make_span_with(telemetry::make_span))
        .layer(request_id::propagate_layer())
        .layer(request_id::set_layer())
        .with_state(state)
}

/// The path the browser asked for. `Router::nest` rewrites the URI
/// handlers and nested layers see, so anything that shows or compares
/// the path reads it from here.
pub fn request_path(extensions: &Extensions, uri: &Uri) -> String {
    extensions
        .get::<OriginalUri>()
        .map_or_else(|| uri.path().to_owned(), |o| o.0.path().to_owned())
}

/// Serve `static_dir` under [`STATIC_PATH`].
pub fn static_service(static_dir: &Path) -> ServeDir {
    ServeDir::new(static_dir)
}
