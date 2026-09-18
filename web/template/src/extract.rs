// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Extractors whose rejections are [`Error`]s, so malformed input gets
//! an error page like everything else. `Query` also runs the type's
//! `Validate` impl; form bodies are validated by the handler instead,
//! which re-renders the form with the messages next to the fields.

use axum::{
    extract::{FromRequest, FromRequestParts, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::Error;

/// `application/x-www-form-urlencoded` body.
#[derive(Debug)]
pub struct Form<T>(pub T);

impl<S, T> FromRequest<S> for Form<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Error> {
        // `csrf::verify` already capped the body at `FORM_LIMIT`.
        let axum::Form(value) =
            axum::Form::<T>::from_request(req, state).await?;
        Ok(Self(value))
    }
}

/// Path segments.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(Error))]
pub struct Path<T>(pub T);

/// Query string, deserialized and validated.
#[derive(Debug)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Error> {
        let axum::extract::Query(value) =
            axum::extract::Query::<T>::from_request_parts(parts, state).await?;
        value
            .validate()
            .map_err(|e| Error::BadRequest(e.to_string()))?;
        Ok(Self(value))
    }
}
