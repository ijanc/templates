// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use utoipa_axum::router::OpenApiRouter;

use crate::AppState;

pub mod items;

/// Mounted under `/v1`.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().nest("/items", items::router())
}
