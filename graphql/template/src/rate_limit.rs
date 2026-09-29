// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Per client rate limiting with `X-RateLimit-*` and `Retry-After`.

use std::{net::IpAddr, time::Duration};

use axum::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
};
use governor::middleware::StateInformationMiddleware;
use tower_governor::{
    GovernorError, GovernorLayer,
    governor::GovernorConfigBuilder,
    key_extractor::{KeyExtractor, PeerIpKeyExtractor, SmartIpKeyExtractor},
};

use crate::{config::RateLimit, error::ApiError};

/// Quota state per client is kept in memory; idle entries are dropped
/// this often.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

/// Where the client address comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// The TCP peer; needs `ConnectInfo<SocketAddr>` on the request.
    Peer,
    /// `X-Forwarded-For`, `X-Real-Ip` or `Forwarded`, falling back to
    /// the peer. Only safe behind a proxy that overwrites those headers.
    Proxy,
}

impl KeyExtractor for Key {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<IpAddr, GovernorError> {
        match self {
            Self::Peer => PeerIpKeyExtractor.extract(req),
            Self::Proxy => SmartIpKeyExtractor.extract(req),
        }
    }
}

/// The layer for `rl`; limited requests get a 429 [`ApiError`].
/// `StateInformationMiddleware` is what adds `x-ratelimit-limit` and
/// `x-ratelimit-remaining` to allowed responses.
pub fn layer(
    rl: &RateLimit,
) -> GovernorLayer<Key, StateInformationMiddleware, Body> {
    let key = if rl.trust_proxy {
        Key::Proxy
    } else {
        Key::Peer
    };
    let conf = GovernorConfigBuilder::default()
        .per_nanosecond(1_000_000_000 / u64::from(rl.per_second.max(1)))
        .burst_size(rl.burst.max(1))
        .key_extractor(key)
        .use_headers()
        .finish()
        .expect("period and burst are non-zero");

    // Every client address gets an entry in the limiter's map, which is
    // never emptied on its own; drop the ones back at full quota.
    let limiter = conf.limiter().clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            tick.tick().await;
            limiter.retain_recent();
        }
    });

    GovernorLayer::new(conf).error_handler(handle)
}

fn handle(e: GovernorError) -> Response {
    match e {
        GovernorError::TooManyRequests { wait_time, headers } => {
            // `retry-after`, `x-ratelimit-after` and friends.
            let mut res = ApiError::TooManyRequests(wait_time).into_response();
            res.headers_mut().extend(headers.unwrap_or_default());
            res
        }
        GovernorError::UnableToExtractKey => {
            ApiError::Internal(anyhow::anyhow!("rate limit: no client address"))
                .into_response()
        }
        GovernorError::Other { code, msg, .. } => ApiError::Internal(
            anyhow::anyhow!("rate limit: {code} {}", msg.unwrap_or_default()),
        )
        .into_response(),
    }
}
