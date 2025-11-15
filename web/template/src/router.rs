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
    extract::{Form, FromRequest, Request, State, rejection::FormRejection},
    http::{self, HeaderName, StatusCode},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use axum_client_ip::{ClientIp, ClientIpSource};
use axum_csrf::{CsrfConfig, CsrfLayer, CsrfToken, Key};
use axum_messages::{Messages, MessagesManagerLayer};
use minijinja::context;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;
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
use validator::Validate;

use crate::metric::track_metrics;
use crate::state::AppState;

const COUNTER_KEY: &str = "counter";
const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Default, Deserialize, Serialize)]
struct Counter(usize);

#[derive(Deserialize, Serialize)]
struct Keys {
    authenticity_token: String,
}

pub(crate) fn route(app_state: Arc<AppState>) -> Router {
    let x_request_id = HeaderName::from_static(REQUEST_ID_HEADER);

    let session_store = MemoryStore::default();
    let cookie_key = Key::generate();
    let config = CsrfConfig::default()
        .with_key(Some(cookie_key))
        .with_cookie_domain(Some("127.0.0.1"));

    // TODO(msi): from config, if debug mode
    let ip_source = ClientIpSource::ConnectInfo;

    Router::new()
        .route("/", get(handler_home))
        .route("/content", get(handler_content))
        .route("/about", get(handler_about))
        .route("/session", get(handler_session))
        .route("/message", get(set_messages_handler))
        .route("/read-messages", get(read_messages_handler))
        .route("/csrf", get(csrf_root).post(csrf_check_key))
        .route("/ip", get(ip_handler))
        .route(
            "/validation",
            get(get_validation_handler).post(post_validation_handler),
        )
        .layer(MessagesManagerLayer)
        // TODO(msi): from config folder asssets
        .nest_service("/assets", ServeDir::new("assets"))
        .layer((
            SetRequestIdLayer::new(x_request_id.clone(), MakeRequestUuid),
            TraceLayer::new_for_http().make_span_with(
                |request: &http::Request<_>| {
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
            CsrfLayer::new(config),
            ip_source.into_extension(),
            // TODO(msi): from config
            TimeoutLayer::new(std::time::Duration::from_secs(10)),
            PropagateRequestIdLayer::new(x_request_id),
        ))
        .route_layer(middleware::from_fn(track_metrics))
        .route("/healthz", get(healthz))
        .with_state(app_state)
}

#[derive(Debug, Deserialize, Validate)]
pub struct NameInput {
    #[validate(length(min = 2, message = "Can not be empty"))]
    pub name: String,
}

async fn get_validation_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ServerError> {
    let template = state.env.get_template("validation").unwrap();

    let rendered = template.render(context! {}).unwrap();

    Ok(Html(rendered))
}

async fn post_validation_handler(
    ValidatedForm(input): ValidatedForm<NameInput>,
) -> Html<String> {
    Html(format!("<h1>Hello, {}!</h1>", input.name))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedForm<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedForm<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    Form<T>: FromRequest<S, Rejection = FormRejection>,
{
    type Rejection = ServerError;

    async fn from_request(
        req: Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Form(value) = Form::<T>::from_request(req, state).await?;
        value.validate()?;
        Ok(ValidatedForm(value))
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),

    #[error(transparent)]
    AxumFormRejection(#[from] FormRejection),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            ServerError::ValidationError(_) => {
                let message = format!("Input validation error: [{self}]")
                    .replace('\n', ", ");
                (StatusCode::BAD_REQUEST, message)
            }
            ServerError::AxumFormRejection(_) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
        }
        .into_response()
    }
}

async fn ip_handler(ClientIp(ip): ClientIp) -> String {
    ip.to_string()
}

async fn csrf_root(
    token: CsrfToken,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let template = state.env.get_template("csrf").unwrap();

    let rendered = template
        .render(context! {
            title => "Csrf",
            authenticity_token => token.authenticity_token().unwrap(),
        })
        .unwrap();
    // We must return the token so that into_response will run and add it to our response cookies.
    (token, Html(rendered)).into_response()
}

async fn csrf_check_key(
    token: CsrfToken,
    Form(payload): Form<Keys>,
) -> &'static str {
    // Verfiy the Hash and return the String message.
    if token.verify(&payload.authenticity_token).is_err() {
        "Token is invalid"
    } else {
        "Token is Valid lets do stuff!"
    }
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
