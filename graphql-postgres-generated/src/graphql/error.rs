// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Resolver errors. Each one becomes a GraphQL error whose
//! `extensions.code` is stable; the cause of an internal error is logged,
//! not sent. `async-graphql` is built with `custom-error-conversion`, so
//! `?` on an `anyhow::Error` goes through [`Error::Internal`] and never
//! leaks the message.

use async_graphql::{ErrorExtensions, Response};
use serde::{Deserialize, Serialize};
use validator::ValidationErrors;

/// `extensions.code` of errors raised by `async-graphql` itself: parse,
/// validation, depth and complexity limits, argument coercion.
pub const BAD_REQUEST: &str = "BAD_REQUEST";

/// Result of a resolver.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything a resolver can fail with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    NotFound(String),
    /// One entry per failed field check, in `extensions.errors`.
    #[error("input validation failed")]
    Validation(Vec<FieldError>),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl Error {
    /// Stable machine readable name of the variant.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NOT_FOUND",
            Self::Validation(_) => "BAD_USER_INPUT",
            Self::Internal(_) => "INTERNAL_SERVER_ERROR",
        }
    }
}

/// One failed check on one input field.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct FieldError {
    pub field: String,
    /// Name of the check that failed.
    pub code: String,
    pub message: String,
}

impl From<Error> for async_graphql::Error {
    fn from(e: Error) -> Self {
        let message = match &e {
            Error::Internal(cause) => {
                tracing::error!(error = format!("{cause:#}"), "internal error");
                "internal server error".to_string()
            }
            e => e.to_string(),
        };
        async_graphql::Error::new(message).extend_with(|_, ext| {
            ext.set("code", e.code());
            if let Error::Validation(errors) = &e {
                ext.set(
                    "errors",
                    async_graphql::to_value(errors).unwrap_or_default(),
                );
            }
        })
    }
}

impl From<ValidationErrors> for Error {
    fn from(e: ValidationErrors) -> Self {
        let mut errors: Vec<FieldError> = e
            .field_errors()
            .into_iter()
            .flat_map(|(field, errs)| {
                errs.iter().map(move |e| FieldError {
                    field: field.to_string(),
                    code: e.code.to_string(),
                    message: e
                        .message
                        .as_deref()
                        .unwrap_or(&e.code)
                        .to_string(),
                })
            })
            .collect();
        errors.sort_by(|a, b| a.field.cmp(&b.field).then(a.code.cmp(&b.code)));
        Self::Validation(errors)
    }
}

/// Completes every error of `res`: [`BAD_REQUEST`] as `code` when there
/// is none, and `request_id` when the request had one.
pub fn finish(res: &mut Response, request_id: Option<&str>) {
    for e in &mut res.errors {
        let ext = e.extensions.get_or_insert_with(Default::default);
        if ext.get("code").is_none() {
            ext.set("code", BAD_REQUEST);
        }
        if let Some(id) = request_id {
            ext.set("request_id", id);
        }
    }
}
