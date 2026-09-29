// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, GRAPHQL_PATH, SDL_PATH, config::RateLimit, error::ApiError,
    graphql, rate_limit, request_id,
};

/// Every route. Only GraphQL requests are rate limited; probes,
/// metrics, GraphiQL and the SDL stay reachable.
pub fn router(rate_limit: Option<RateLimit>) -> Router<AppState> {
    let execute = post(execute);
    let execute = match rate_limit {
        Some(rl) => execute.layer(rate_limit::layer(&rl)),
        None => execute,
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route(GRAPHQL_PATH, get(graphiql).merge(execute))
        .route(SDL_PATH, get(sdl))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Health {
    pub status: String,
    pub version: String,
}

impl Health {
    fn ok() -> Self {
        Self {
            status: "ok".into(),
            version: crate::version(),
        }
    }
}

/// Liveness: the process is up.
pub async fn healthz() -> Json<Health> {
    Json(Health::ok())
}

/// Readiness: the store answers.
pub async fn readyz(
    State(state): State<AppState>,
) -> Result<Json<Health>, ApiError> {
    state
        .store
        .ping()
        .await
        .map_err(|e| ApiError::Unavailable(format!("store: {e:#}")))?;
    Ok(Json(Health::ok()))
}

/// Run one GraphQL request. Errors inside it are in the response body
/// with status 200; only a body that is not a GraphQL request is a 4xx.
pub async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: GraphQLRequest<ApiError>,
) -> GraphQLResponse {
    let mut res = state.schema.execute(req.into_inner()).await;
    graphql::error::finish(&mut res, request_id::get(&headers).as_deref());
    res.into()
}

/// GraphiQL, pointed at [`GRAPHQL_PATH`].
pub async fn graphiql() -> Html<String> {
    Html(GraphiQLSource::build().endpoint(GRAPHQL_PATH).finish())
}

/// The schema in SDL, as plain text.
pub async fn sdl(State(state): State<AppState>) -> String {
    state.schema.sdl()
}
