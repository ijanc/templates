// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::headers::{IfMatch, IfNoneMatch};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    AppState,
    dto::{CreateItem, Item, ListQuery, Page, UpdateItem},
    error::{ApiError, Problem},
    extract::{Header, Json, Path, Query},
    http_cache,
};

/// Mounted under `/v1/items`.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(create, list))
        .routes(routes!(get, update, delete))
}

/// 412 unless `If-Match` is absent or names the current item. The
/// window between this read and the write is accepted.
async fn check_if_match(
    state: &AppState,
    id: Uuid,
    if_match: Option<IfMatch>,
) -> Result<(), ApiError> {
    let Some(if_match) = if_match else {
        return Ok(());
    };
    let item = state.items.get(id).await?;
    if if_match.precondition_passes(&http_cache::etag(&item)) {
        Ok(())
    } else {
        Err(ApiError::PreconditionFailed(format!(
            "item {id} has changed"
        )))
    }
}

/// Create an item.
#[utoipa::path(
    post,
    path = "",
    tag = "items",
    request_body = CreateItem,
    responses(
        (status = 201, body = Item, headers(
            ("Location" = String),
            ("ETag" = String),
            ("Cache-Control" = String),
        )),
        (status = 400, body = Problem),
        (status = 422, body = Problem),
        (status = 429, body = Problem),
    ),
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateItem>,
) -> Result<
    (
        StatusCode,
        HeaderMap,
        [(header::HeaderName, String); 1],
        Json<Item>,
    ),
    ApiError,
> {
    let item = state.items.create(input.into()).await?;
    let location = format!("/v1/items/{}", item.id);
    Ok((
        StatusCode::CREATED,
        http_cache::headers(&item),
        [(header::LOCATION, location)],
        Json(item.into()),
    ))
}

/// List items, oldest first.
#[utoipa::path(
    get,
    path = "",
    tag = "items",
    params(ListQuery),
    responses(
        (status = 200, body = Page),
        (status = 400, body = Problem),
        (status = 422, body = Problem),
        (status = 429, body = Problem),
    ),
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Page>, ApiError> {
    Ok(Json(state.items.list(q.into()).await?.into()))
}

/// Fetch one item; `304` when `If-None-Match` names the current one.
#[utoipa::path(
    get,
    path = "/{id}",
    tag = "items",
    params(
        ("id" = Uuid, Path, description = "Item id"),
        ("If-None-Match" = Option<String>, Header,
            description = "ETag of the cached copy"),
    ),
    responses(
        (status = 200, body = Item, headers(
            ("ETag" = String),
            ("Cache-Control" = String),
        )),
        (status = 304, headers(
            ("ETag" = String),
            ("Cache-Control" = String),
        )),
        (status = 400, body = Problem),
        (status = 404, body = Problem),
        (status = 429, body = Problem),
    ),
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Header(if_none_match): Header<IfNoneMatch>,
) -> Result<Response, ApiError> {
    let item = state.items.get(id).await?;
    let headers = http_cache::headers(&item);
    let not_modified = if_none_match
        .is_some_and(|h| !h.precondition_passes(&http_cache::etag(&item)));
    if not_modified {
        Ok((StatusCode::NOT_MODIFIED, headers).into_response())
    } else {
        Ok((headers, Json(Item::from(item))).into_response())
    }
}

/// Replace an item; `412` when `If-Match` names another version.
#[utoipa::path(
    put,
    path = "/{id}",
    tag = "items",
    params(
        ("id" = Uuid, Path, description = "Item id"),
        ("If-Match" = Option<String>, Header,
            description = "ETag the update is based on"),
    ),
    request_body = UpdateItem,
    responses(
        (status = 200, body = Item, headers(
            ("ETag" = String),
            ("Cache-Control" = String),
        )),
        (status = 400, body = Problem),
        (status = 404, body = Problem),
        (status = 412, body = Problem),
        (status = 422, body = Problem),
        (status = 429, body = Problem),
    ),
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Header(if_match): Header<IfMatch>,
    Json(input): Json<UpdateItem>,
) -> Result<(HeaderMap, Json<Item>), ApiError> {
    check_if_match(&state, id, if_match).await?;
    let item = state.items.update(id, input.into()).await?;
    Ok((http_cache::headers(&item), Json(item.into())))
}

/// Delete an item; `412` when `If-Match` names another version.
#[utoipa::path(
    delete,
    path = "/{id}",
    tag = "items",
    params(
        ("id" = Uuid, Path, description = "Item id"),
        ("If-Match" = Option<String>, Header,
            description = "ETag the delete is based on"),
    ),
    responses(
        (status = 204),
        (status = 400, body = Problem),
        (status = 404, body = Problem),
        (status = 412, body = Problem),
        (status = 429, body = Problem),
    ),
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Header(if_match): Header<IfMatch>,
) -> Result<StatusCode, ApiError> {
    check_if_match(&state, id, if_match).await?;
    state.items.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
