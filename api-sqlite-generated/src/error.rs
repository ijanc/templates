// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Error responses as RFC 9457 Problem Details.

use axum::{
    body::Body,
    extract::{
        Request,
        rejection::{
            BytesRejection, FailedToBufferBody, JsonRejection, PathRejection,
            QueryRejection,
        },
    },
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::ValidationErrors;

use crate::request_id;

/// Media type of every error body.
pub const PROBLEM_JSON: HeaderValue =
    HeaderValue::from_static("application/problem+json");

/// Prefix of [`Problem::type`]; the [`Problem::code`] follows.
pub const TYPE_PREFIX: &str = "urn:api_sqlite_generated:error:";

/// Everything a handler can fail with; each variant maps to a status.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 400
    #[error("{0}")]
    BadRequest(String),
    /// 404
    #[error("{0}")]
    NotFound(String),
    /// 412; `If-Match` did not match the current representation.
    #[error("precondition failed: {0}")]
    PreconditionFailed(String),
    /// 413
    #[error("request body too large")]
    PayloadTooLarge,
    /// 422; one entry per failed field check.
    #[error("request validation failed")]
    Validation(Vec<FieldError>),
    /// 429; the client may retry after this many seconds.
    #[error("rate limit exceeded, retry in {0}s")]
    TooManyRequests(u64),
    /// 503
    #[error("{0}")]
    Unavailable(String),
    /// 500; the cause is logged, not sent.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::PreconditionFailed(_) => StatusCode::PRECONDITION_FAILED,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable machine readable name of the variant.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::NotFound(_) => "not_found",
            Self::PreconditionFailed(_) => "precondition_failed",
            Self::PayloadTooLarge => "payload_too_large",
            Self::Validation(_) => "validation",
            Self::TooManyRequests(_) => "too_many_requests",
            Self::Unavailable(_) => "unavailable",
            Self::Internal(_) => "internal",
        }
    }
}

/// One failed check on one field of a 422.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
pub struct FieldError {
    #[schema(example = "name")]
    pub field: String,
    /// Name of the check that failed.
    #[schema(example = "length")]
    pub code: String,
    #[schema(example = "name must be at most 200 characters")]
    pub message: String,
}

/// Body of every error response, `application/problem+json` (RFC 9457).
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Problem {
    /// URI identifying the error kind: [`TYPE_PREFIX`] plus `code`.
    #[schema(example = "urn:api_sqlite_generated:error:not_found")]
    pub r#type: String,
    /// Reason phrase of `status`.
    #[schema(example = "Not Found")]
    pub title: String,
    #[schema(example = 404)]
    pub status: u16,
    #[schema(example = "item not found")]
    pub detail: String,
    /// Path of the failed request.
    #[schema(example = "/v1/items/9f0c")]
    pub instance: Option<String>,
    /// See [`ApiError::code`].
    #[schema(example = "not_found")]
    pub code: String,
    /// The `X-Request-Id` of the failed request.
    pub request_id: Option<String>,
    /// Field checks that failed; 422 only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FieldError>>,
}

impl Problem {
    fn new(status: StatusCode, code: &str, detail: String) -> Self {
        Self {
            r#type: format!("{TYPE_PREFIX}{code}"),
            title: status.canonical_reason().unwrap_or("Error").into(),
            status: status.as_u16(),
            detail,
            instance: None,
            code: code.into(),
            request_id: None,
            errors: None,
        }
    }

    /// A body for a bare status, e.g. a 405 from the router; `code` is
    /// the reason phrase in snake case.
    pub fn from_status(status: StatusCode) -> Self {
        let reason = status.canonical_reason().unwrap_or("error");
        let code = reason.to_lowercase().replace([' ', '-'], "_");
        Self::new(status, &code, reason.to_lowercase())
    }

    fn into_response_with(self, mut res: Response) -> Response {
        let body = serde_json::to_vec(&self).unwrap_or_default();
        let headers = res.headers_mut();
        headers.insert(header::CONTENT_TYPE, PROBLEM_JSON);
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
        *res.body_mut() = Body::from(body);
        res
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let detail = match &self {
            Self::Internal(e) => {
                tracing::error!(error = format!("{e:#}"), "internal error");
                "internal server error".to_string()
            }
            e => e.to_string(),
        };
        let status = self.status();
        let mut body = Problem::new(status, self.code(), detail);
        if let Self::Validation(errors) = self {
            body.errors = Some(errors);
        }
        let mut res = Response::new(Body::empty());
        *res.status_mut() = status;
        // Picked up by `problem_middleware`.
        res.extensions_mut().insert(body.clone());
        body.into_response_with(res)
    }
}

/// Middleware completing error bodies. Errors are built without access
/// to the request, so this runs afterwards, fills `request_id` and
/// `instance` into bodies tagged by [`ApiError::into_response`], and
/// gives bodiless 4xx/5xx responses (405 from the router, 408 from the
/// timeout) a [`Problem`] too. Headers are kept as they are.
pub async fn problem_middleware(req: Request, next: Next) -> Response {
    let id = request_id::get(req.headers());
    let path = req.uri().path().to_owned();
    let mut res = next.run(req).await;
    let mut body = match res.extensions_mut().remove::<Problem>() {
        Some(body) => body,
        None if (res.status().is_client_error()
            || res.status().is_server_error())
            && !res.headers().contains_key(header::CONTENT_TYPE) =>
        {
            Problem::from_status(res.status())
        }
        None => return res,
    };
    body.request_id = id;
    body.instance = Some(path);
    body.into_response_with(res)
}

/// Fallback handler for unknown routes.
pub async fn not_found() -> ApiError {
    ApiError::NotFound("route not found".into())
}

impl From<ValidationErrors> for ApiError {
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

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        match r {
            JsonRejection::JsonDataError(e) => {
                Self::Validation(vec![FieldError {
                    field: "body".into(),
                    code: "invalid".into(),
                    message: e.body_text(),
                }])
            }
            JsonRejection::BytesRejection(
                BytesRejection::FailedToBufferBody(
                    FailedToBufferBody::LengthLimitError(_),
                ),
            ) => Self::PayloadTooLarge,
            other => Self::BadRequest(other.body_text()),
        }
    }
}

impl From<PathRejection> for ApiError {
    fn from(r: PathRejection) -> Self {
        Self::BadRequest(r.body_text())
    }
}

impl From<QueryRejection> for ApiError {
    fn from(r: QueryRejection) -> Self {
        Self::BadRequest(r.body_text())
    }
}
