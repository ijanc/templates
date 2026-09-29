// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use axum::http::Request;

use crate::request_id;

/// Span for one request, carrying its id so every event inside can be
/// correlated with the `X-Request-Id` header.
pub fn make_span<B>(req: &Request<B>) -> tracing::Span {
    let request_id = request_id::get(req.headers()).unwrap_or_default();
    tracing::info_span!(
        "request",
        method = %req.method(),
        uri = %req.uri(),
        request_id = %request_id,
    )
}
