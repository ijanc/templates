// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use axum::http::{HeaderMap, HeaderName};
use tower_http::request_id::{
    MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer,
};

/// Header carrying the request id, both ways.
pub const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Keeps a client supplied `X-Request-Id`, generates a UUID v4 otherwise.
pub fn set_layer() -> SetRequestIdLayer<MakeRequestUuid> {
    SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid)
}

/// Copies the request id to the response.
pub fn propagate_layer() -> PropagateRequestIdLayer {
    PropagateRequestIdLayer::new(X_REQUEST_ID)
}

/// The request id from `headers`, if present and printable.
pub fn get(headers: &HeaderMap) -> Option<String> {
    headers
        .get(X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}
