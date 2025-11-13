//
// Copyright (c) 2025 murilo ijanc' <murilo@ijanc.org>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//
use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use minijinja::context;
use tower_http::{
    services::ServeDir, timeout::TimeoutLayer, trace::TraceLayer,
};

use crate::state::AppState;

pub(crate) fn route(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(handler_home))
        .route("/content", get(handler_content))
        .route("/about", get(handler_about))
        // TODO(msi): from config folder asssets
        .nest_service("/assets", ServeDir::new("assets"))
        .layer((
            TraceLayer::new_for_http(),
            // TODO(msi): from config
            TimeoutLayer::new(Duration::from_secs(10)),
        ))
        .with_state(app_state)
}

async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

async fn handler_home(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let template = state.env.get_template("home").unwrap();

    let rendered = template
        .render(context! {
            title => "Home",
            welcome_text => "Hello World!",
        })
        .unwrap();

    Ok(Html(rendered))
}

async fn handler_content(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let template = state.env.get_template("content").unwrap();

    let some_example_entries = vec!["Data 1", "Data 2", "Data 3"];

    let rendered = template
        .render(context! {
            title => "Content",
            entries => some_example_entries,
        })
        .unwrap();

    Ok(Html(rendered))
}

async fn handler_about(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, StatusCode> {
    let template = state.env.get_template("about").unwrap();

    let rendered = template.render(context!{
        title => "About",
        about_text => "Simple demonstration layout for an axum project with minijinja as templating engine.",
    }).unwrap();

    Ok(Html(rendered))
}
