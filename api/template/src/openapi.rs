// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use utoipa::OpenApi;

use crate::error::{FieldError, Problem};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "{{project-name}}",
        description = "{{project-description}}",
        license(name = "ISC"),
    ),
    tags(
        (name = "health", description = "Liveness and readiness"),
        (name = "items", description = "Item CRUD"),
    ),
    components(schemas(Problem, FieldError)),
)]
struct ApiDoc;

/// The base document; routes are added by `OpenApiRouter`.
pub fn doc() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    doc.info.version = crate::version();
    doc
}
