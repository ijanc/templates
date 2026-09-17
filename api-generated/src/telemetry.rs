// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use anyhow::Context;
use axum::http::Request;
use tracing_subscriber::EnvFilter;

use crate::{
    config::{Config, ENV_PREFIX, LogFormat},
    request_id,
};

/// Install the global `tracing` subscriber.
pub fn init(cfg: &Config) -> anyhow::Result<()> {
    let filter = EnvFilter::try_new(&cfg.log)
        .with_context(|| format!("{ENV_PREFIX}_LOG: invalid filter"))?;
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match cfg.log_format {
        LogFormat::Pretty => builder.init(),
        LogFormat::Json => builder.json().flatten_event(true).init(),
    }
    Ok(())
}

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
