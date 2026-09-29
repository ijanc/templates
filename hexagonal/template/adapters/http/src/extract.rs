// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Extractors whose rejections are [`ApiError`]s, so malformed input
//! gets the same Problem Details body as everything else. They check
//! shape only; the rules live in the domain, and breaking one is a 422
//! from the use case, not from here. `Header` reads an optional typed
//! header.

use axum::{
    extract::{FromRequest, FromRequestParts},
    http::request::Parts,
};
use axum_extra::headers::{self, HeaderMapExt};
use serde::Serialize;

use crate::error::ApiError;

/// JSON request body.
#[derive(Debug, FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(pub T);

impl<T: Serialize> axum::response::IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

/// Path segments.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Path<T>(pub T);

/// Query string.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

/// A typed request header; `None` when absent, 400 when malformed.
#[derive(Debug)]
pub struct Header<T>(pub Option<T>);

impl<S, T> FromRequestParts<S> for Header<T>
where
    S: Send + Sync,
    T: headers::Header,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, ApiError> {
        parts.headers.typed_try_get::<T>().map(Self).map_err(|e| {
            ApiError::BadRequest(format!("{}: {e}", T::name().as_str()))
        })
    }
}
