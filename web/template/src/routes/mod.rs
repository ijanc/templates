// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use std::path::Path;

use axum::{
    Json, Router,
    extract::State,
    response::Redirect,
    routing::{get, get_service},
};
use serde::{Deserialize, Serialize};

use crate::{AppState, STATIC_PATH, error::Error};

{%- if auth != "none" %}

pub mod auth;
{%- endif %}
pub mod items;

/// Every route, plus the static file service.
pub fn router(state: &AppState, static_dir: &Path) -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .nest("/items", items::router(state))
{%- if auth != "none" %}
        .merge(auth::router(state))
{%- endif %}
        .nest_service(
            STATIC_PATH,
            get_service(crate::static_service(static_dir)),
        )
}

/// The site starts at the item list.
pub async fn index() -> Redirect {
    Redirect::to("/items")
}

/// Answer of the probes; JSON, so a monitor does not parse HTML.
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
) -> Result<Json<Health>, Error> {
    state
        .store
        .ping()
        .await
        .map_err(|e| Error::Unavailable(format!("store: {e:#}")))?;
    Ok(Json(Health::ok()))
}
