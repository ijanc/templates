// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Driving adapter: the HTTP API on axum. Handlers turn requests into
//! calls on [`ItemUseCases`] and results into responses; nothing here
//! touches a store.

use std::{sync::Arc, time::Duration};

use application::ItemUseCases;
use axum::{Router, http::StatusCode, middleware};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

pub mod dto;
pub mod error;
pub mod extract;
pub mod http_cache;
pub mod metrics;
pub mod openapi;
pub mod rate_limit;
pub mod request_id;
pub mod routes;
pub mod trace;

pub use rate_limit::RateLimit;

/// Path of the OpenAPI document.
pub const OPENAPI_PATH: &str = "/api-docs/openapi.json";

/// Path of the Swagger UI.
pub const SWAGGER_PATH: &str = "/swagger-ui";

/// Requests taking longer than this get a 408.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    /// The use cases behind `/v1`.
    pub items: Arc<dyn ItemUseCases>,
    /// Reported by the probes and the OpenAPI document.
    pub version: String,
}

/// The complete router: API routes, OpenAPI document, Swagger UI,
/// metrics and the middleware stack. `rate_limit` applies to `/v1`;
/// `None` disables it. Serve with
/// `into_make_service_with_connect_info::<SocketAddr>()`, the limiter
/// keys on the peer address.
pub fn app(state: AppState, rate_limit: Option<RateLimit>) -> Router {
    let doc = openapi::doc(&state.version);
    let (router, api) = OpenApiRouter::with_openapi(doc)
        .merge(routes::router(rate_limit))
        .split_for_parts();
    router
        .merge(SwaggerUi::new(SWAGGER_PATH).url(OPENAPI_PATH, api))
        .merge(metrics::router())
        .fallback(error::not_found)
        // Innermost first: the timeout's 408 passes through the problem
        // middleware, and the request id is set last, so it exists
        // before the trace span and the problem middleware look at it.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(middleware::from_fn(error::problem_middleware))
        .layer(metrics::layer())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http().make_span_with(trace::make_span))
        .layer(request_id::propagate_layer())
        .layer(request_id::set_layer())
        .with_state(state)
}
