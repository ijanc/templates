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
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{HeaderName, Request, StatusCode},
    middleware,
    response::{Html, IntoResponse, Redirect},
    routing::get,
};
use axum_messages::{Messages, MessagesManagerLayer};
use minijinja::context;
use serde::{Deserialize, Serialize};
use time::Duration;
use tower_http::{
    request_id::{
        MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer,
    },
    services::ServeDir,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};
use tracing::{error, info_span};

use crate::metric::track_metrics;
use crate::state::AppState;

const COUNTER_KEY: &str = "counter";
const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Default, Deserialize, Serialize)]
struct Counter(usize);

pub(crate) fn route(app_state: Arc<AppState>) -> Router {
    let x_request_id = HeaderName::from_static(REQUEST_ID_HEADER);

    let session_store = MemoryStore::default();

    Router::new()
        .route("/", get(handler_home))
        .route("/content", get(handler_content))
        .route("/about", get(handler_about))
        .route("/session", get(handler_session))
        .route("/message", get(set_messages_handler))
        .route("/read-messages", get(read_messages_handler))
        .layer(MessagesManagerLayer)
        // TODO(msi): from config folder asssets
        .nest_service("/assets", ServeDir::new("assets"))
        .layer((
            SetRequestIdLayer::new(x_request_id.clone(), MakeRequestUuid),
            TraceLayer::new_for_http().make_span_with(
                |request: &Request<_>| {
                    // Log the request id as generated.
                    let request_id = request.headers().get(REQUEST_ID_HEADER);

                    match request_id {
                        Some(request_id) => info_span!(
                            "http_request",
                            request_id = ?request_id,
                        ),
                        None => {
                            error!("could not extract request_id");
                            info_span!("http_request")
                        }
                    }
                },
            ),
            SessionManagerLayer::new(session_store)
                .with_secure(false)
                .with_expiry(Expiry::OnInactivity(Duration::seconds(10))),
            MessagesManagerLayer,
            // TODO(msi): from config
            TimeoutLayer::new(std::time::Duration::from_secs(10)),
            PropagateRequestIdLayer::new(x_request_id),
        ))
        .route_layer(middleware::from_fn(track_metrics))
        .route("/healthz", get(healthz))
        .with_state(app_state)
}

async fn set_messages_handler(messages: Messages) -> impl IntoResponse {
    messages.info("Hello, world!").debug("This is a debug message.");

    Redirect::to("/read-messages")
}

async fn read_messages_handler(messages: Messages) -> impl IntoResponse {
    let messages = messages
        .into_iter()
        .map(|message| format!("{}: {}", message.level, message))
        .collect::<Vec<_>>()
        .join(", ");

    if messages.is_empty() { "No messages yet!".to_string() } else { messages }
}

async fn handler_session(session: Session) -> impl IntoResponse {
    let counter: Counter =
        session.get(COUNTER_KEY).await.unwrap().unwrap_or_default();
    session.insert(COUNTER_KEY, counter.0 + 1).await.unwrap();
    format!("Current count: {}", counter.0)
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
