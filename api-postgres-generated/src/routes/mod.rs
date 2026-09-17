// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    config::RateLimit,
    error::{ApiError, Problem},
    extract::Json,
    rate_limit,
};

pub mod v1;

/// Every route, unversioned probes plus `/v1`. Only `/v1` is rate
/// limited; probes, metrics and docs stay reachable.
pub fn router(rate_limit: Option<RateLimit>) -> OpenApiRouter<AppState> {
    let v1 = v1::router();
    let v1 = match rate_limit {
        Some(rl) => v1.layer(rate_limit::layer(&rl)),
        None => v1,
    };
    OpenApiRouter::new()
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        .nest("/v1", v1)
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct Health {
    #[schema(example = "ok")]
    pub status: String,
    #[schema(example = "0.1.0 (abc1234 2026-01-31)")]
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
#[utoipa::path(
    get,
    path = "/healthz",
    tag = "health",
    responses((status = 200, body = Health)),
)]
pub async fn healthz() -> Json<Health> {
    Json(Health::ok())
}

/// Readiness: the store answers.
#[utoipa::path(
    get,
    path = "/readyz",
    tag = "health",
    responses(
        (status = 200, body = Health),
        (status = 503, body = Problem),
    ),
)]
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
