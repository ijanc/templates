// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Cookie backed sessions. They carry the CSRF token, the pending flash
//! message and the id of the logged in user.

use std::time::Duration;

use axum::extract::Request;
use tower_sessions::{
    Expiry, Session, SessionManagerLayer, cookie::SameSite,
    session_store::ExpiredDeletion,
};

use crate::{error::Error, store::Store};

/// Name of the session cookie.
pub const COOKIE: &str = "web_generated_session";

/// How often expired rows are swept from the database.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(600);

/// Where sessions are kept.
pub type SessionStore = tower_sessions_sqlx_store::SqliteStore;

/// The layer [`crate::app`] wraps the router in.
pub type Layer = SessionManagerLayer<SessionStore>;

/// Cookie lifetime and flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Idle time after which a session is dropped.
    pub ttl: Duration,
    /// Send the cookie over HTTPS only; turn it on in production.
    pub secure: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(86_400),
            secure: false,
        }
    }
}

/// A session store sharing the connection pool of `store`, with its
/// table created if missing.
pub async fn store(store: &Store) -> anyhow::Result<SessionStore> {
    let sessions = SessionStore::new(store.pool().clone());
    sessions.migrate().await?;
    Ok(sessions)
}

/// The session of `req`; [`layer`] puts one on every request, so this
/// only fails when the layer is missing.
pub fn get(req: &Request) -> Result<Session, Error> {
    req.extensions()
        .get::<Session>()
        .cloned()
        .ok_or_else(|| Error::Internal(anyhow::anyhow!("no session layer")))
}

/// Wrap the router so every request has a session.
pub fn layer(store: SessionStore, cfg: &Config) -> Layer {
    let ttl = time::Duration::try_from(cfg.ttl)
        .unwrap_or_else(|_| time::Duration::days(1));
    SessionManagerLayer::new(store)
        .with_name(COOKIE)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(cfg.secure)
        .with_expiry(Expiry::OnInactivity(ttl))
}

/// Delete expired sessions in the background, forever.
pub fn sweep_expired(store: SessionStore) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = store.continuously_delete_expired(SWEEP_INTERVAL).await
        {
            tracing::error!(error = %e, "session sweep stopped");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_a_day_and_insecure() {
        let cfg = Config::default();
        assert_eq!(cfg.ttl, Duration::from_secs(86_400));
        assert!(!cfg.secure);
    }
}
