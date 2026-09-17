// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Extractors whose rejections are [`ApiError`]s, so malformed input
//! gets the same Problem Details body as everything else. `Json` and
//! `Query` also run the type's `Validate` impl; failures are a 422 with
//! one entry per field. `Header` reads an optional typed header.

use axum::{
    extract::{FromRequest, FromRequestParts, Request},
    http::request::Parts,
};
use axum_extra::headers::{self, HeaderMapExt};
use serde::{Serialize, de::DeserializeOwned};
use validator::Validate;

use crate::error::ApiError;

/// JSON request body, deserialized and validated.
#[derive(Debug)]
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        let axum::Json(value) =
            axum::Json::<T>::from_request(req, state).await?;
        value.validate()?;
        Ok(Self(value))
    }
}

impl<T: Serialize> axum::response::IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

/// Path segments.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Path<T>(pub T);

/// Query string, deserialized and validated.
#[derive(Debug)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, ApiError> {
        let axum::extract::Query(value) =
            axum::extract::Query::<T>::from_request_parts(parts, state).await?;
        value.validate()?;
        Ok(Self(value))
    }
}

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
