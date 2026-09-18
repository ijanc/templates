// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Failures as HTML pages. Handlers return [`Error`]; the response it
//! builds carries only a status and a tag, and
//! [`page_middleware`] turns that into a rendered page.

use axum::{
    body::Body,
    extract::{
        Request, State,
        rejection::{
            BytesRejection, FailedToBufferBody, FormRejection, PathRejection,
            QueryRejection,
        },
    },
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use minijinja::context;

use crate::{AppState, render::Templates, request_id};

/// Content type of every rendered page.
pub const TEXT_HTML: HeaderValue =
    HeaderValue::from_static("text/html; charset=utf-8");

/// Everything a handler can fail with; each variant maps to a status.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 400
    #[error("{0}")]
    BadRequest(String),
    /// 403
    #[error("{0}")]
    Forbidden(String),
    /// 404
    #[error("{0}")]
    NotFound(String),
    /// 413
    #[error("request body too large")]
    PayloadTooLarge,
    /// 503
    #[error("{0}")]
    Unavailable(String),
    /// 500; the cause is logged, not shown.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Status and user-safe detail of a failed request, attached to the
/// response for [`page_middleware`] to render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorPage {
    pub status: StatusCode,
    pub detail: String,
}

impl ErrorPage {
    /// A page for a bare status, e.g. a 405 from the router.
    pub fn from_status(status: StatusCode) -> Self {
        let reason = status.canonical_reason().unwrap_or("error");
        Self {
            status,
            detail: reason.to_lowercase(),
        }
    }

    /// `errors/<status>.html` first, `errors/error.html` otherwise.
    fn render(
        &self,
        templates: &Templates,
        path: &str,
        id: Option<&str>,
    ) -> String {
        let status = self.status;
        let ctx = context! {
            app_name => crate::PROG,
            version => crate::version(),
            auth => crate::AUTH,
            auth_google => crate::AUTH_GOOGLE,
            path => path,
            request_id => id,
            status => status.as_u16(),
            reason => status.canonical_reason().unwrap_or("Error"),
            detail => self.detail,
        };
        let named = format!("errors/{}.html", status.as_u16());
        templates
            .render_any(&[&named, "errors/error.html"], ctx)
            .unwrap_or_else(|e| {
                tracing::error!(error = format!("{e:#}"), "error page failed");
                format!(
                    "<!doctype html><title>{status}</title><h1>{status}</h1>"
                )
            })
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let detail = match &self {
            Self::Internal(e) => {
                tracing::error!(error = format!("{e:#}"), "internal error");
                "internal server error".to_string()
            }
            e => e.to_string(),
        };
        let status = self.status();
        let mut res = Response::new(Body::empty());
        *res.status_mut() = status;
        // Picked up by `page_middleware`.
        res.extensions_mut().insert(ErrorPage { status, detail });
        res
    }
}

/// Middleware rendering error pages. Errors are built without access to
/// the request, so this runs afterwards, renders responses tagged by
/// [`Error::into_response`], and gives bodiless 4xx/5xx responses (404
/// and 405 from the router, 408 from the timeout) a page too.
pub async fn page_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let id = request_id::get(req.headers());
    let path = req.uri().path().to_owned();
    let mut res = next.run(req).await;
    let page = match res.extensions_mut().remove::<ErrorPage>() {
        Some(page) => page,
        None if (res.status().is_client_error()
            || res.status().is_server_error())
            && !res.headers().contains_key(header::CONTENT_TYPE) =>
        {
            ErrorPage::from_status(res.status())
        }
        None => return res,
    };
    let body = page.render(&state.templates, &path, id.as_deref());
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, TEXT_HTML);
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
    *res.body_mut() = Body::from(body);
    res
}

/// Fallback handler for unknown routes.
pub async fn not_found() -> Error {
    Error::NotFound("page not found".into())
}

impl From<tower_sessions::session::Error> for Error {
    fn from(e: tower_sessions::session::Error) -> Self {
        Self::Internal(e.into())
    }
}

impl From<FormRejection> for Error {
    fn from(r: FormRejection) -> Self {
        match r {
            FormRejection::FailedToDeserializeForm(e) => {
                Self::BadRequest(e.body_text())
            }
            FormRejection::BytesRejection(
                BytesRejection::FailedToBufferBody(
                    FailedToBufferBody::LengthLimitError(_),
                ),
            ) => Self::PayloadTooLarge,
            other => Self::BadRequest(other.body_text()),
        }
    }
}

impl From<PathRejection> for Error {
    fn from(r: PathRejection) -> Self {
        Self::BadRequest(r.body_text())
    }
}

impl From<QueryRejection> for Error {
    fn from(r: QueryRejection) -> Self {
        Self::BadRequest(r.body_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_match_variants() {
        assert_eq!(
            Error::BadRequest("x".into()).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            Error::Forbidden("x".into()).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(Error::NotFound("x".into()).status(), StatusCode::NOT_FOUND);
        assert_eq!(
            Error::PayloadTooLarge.status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            Error::Unavailable("x".into()).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let e = Error::Internal(anyhow::anyhow!("boom"));
        assert_eq!(e.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn bare_status_becomes_a_page() {
        let page = ErrorPage::from_status(StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(page.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(page.detail, "method not allowed");
    }
}
