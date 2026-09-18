// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Accounts, sessions and the guard around the pages that write.
//!
//! Registration is invite only: a logged in user creates a token at
//! `/invites` and shares `/register?token=<token>`. The very first
//! account is created with `<PREFIX>_BOOTSTRAP_INVITE`, which counts as
//! a token for as long as no user exists.

use std::time::Duration;

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::Error as HashError,
};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::request::Parts,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};
use tower_sessions::Session;
use uuid::Uuid;

use crate::{
    AppState,
    error::Error,
    model::{Invite, User},
    session,
    store::Store,
};

/// Session key holding the id of the logged in user.
pub const SESSION_KEY: &str = "user_id";

/// Where an anonymous visitor is sent, with `?next=` appended.
pub const LOGIN_PATH: &str = "/login";

/// Shown whichever field was wrong, so the form never confirms that an
/// email exists.
pub const BAD_CREDENTIALS: &str = "email or password is not right";

/// What the auth routes need from the environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// How long a new invite stays usable.
    pub invite_ttl: Duration,
    /// Token accepted while no account exists.
    pub bootstrap_invite: Option<String>,
}

/// The logged in user, `None` when anonymous. A session naming a user
/// that no longer exists is treated as anonymous.
#[derive(Debug)]
pub struct CurrentUser(pub Option<User>);

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Error> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(_, e)| Error::Internal(anyhow::anyhow!("{e}")))?;
        let app: AppState = axum::extract::FromRef::from_ref(state);
        Ok(Self(current(&session, &app.store).await?))
    }
}

/// The user named by `session`, if the account is still there.
pub async fn current(
    session: &Session,
    store: &Store,
) -> Result<Option<User>, Error> {
    let id = session.get::<Uuid>(SESSION_KEY).await?;
    match id {
        Some(id) => Ok(store.user(id).await?),
        None => Ok(None),
    }
}

/// Bind the session to `user`; the id is cycled so a session fixated
/// before the login cannot be reused after it.
pub async fn login(session: &Session, user: &User) -> Result<(), Error> {
    session.cycle_id().await?;
    Ok(session.insert(SESSION_KEY, user.id).await?)
}

/// Drop the session, and with it the login, flash and CSRF token.
pub async fn logout(session: &Session) -> Result<(), Error> {
    Ok(session.flush().await?)
}

/// Send anonymous visitors to the login page, remembering where they
/// were headed.
pub async fn require_auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Error> {
    let session = session::get(&req)?;
    if current(&session, &state.store).await?.is_some() {
        return Ok(next.run(req).await);
    }
    let path = crate::request_path(req.extensions(), req.uri());
    Ok(Redirect::to(&login_url(&path)).into_response())
}

/// `/login?next=<path>`, or plain `/login` for the login page itself.
pub fn login_url(next: &str) -> String {
    if next == LOGIN_PATH || !next.starts_with('/') {
        return LOGIN_PATH.to_string();
    }
    let query =
        serde_urlencoded::to_string([("next", next)]).unwrap_or_default();
    format!("{LOGIN_PATH}?{query}")
}

/// Only in-app paths are followed after a login, so `?next=` cannot
/// bounce anyone to another site.
pub fn safe_next(next: Option<&str>) -> Option<String> {
    next.filter(|n| n.starts_with('/') && !n.starts_with("//"))
        .map(str::to_owned)
}

/// A PHC string; the salt comes from the operating system.
pub fn hash_password(password: &str) -> Result<String, Error> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|e| Error::Internal(anyhow::anyhow!("hash password: {e}")))
}

/// Whether `password` matches `hash`. A stored hash that does not parse
/// is a failure, not a pass.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, Error> {
    let parsed = match PasswordHash::new(hash) {
        Ok(parsed) => parsed,
        Err(e) => {
            tracing::error!(error = %e, "stored password hash is invalid");
            return Ok(false);
        }
    };
    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(HashError::PasswordInvalid) => Ok(false),
        Err(e) => Err(Error::Internal(anyhow::anyhow!("verify: {e}"))),
    }
}

/// A fresh invite, valid for `ttl` from now.
pub fn new_invite(
    created_by: Uuid,
    email: Option<String>,
    ttl: Duration,
    now: DateTime<Utc>,
) -> Invite {
    let ttl = chrono::Duration::from_std(ttl)
        .unwrap_or_else(|_| chrono::Duration::days(7));
    Invite {
        token: new_token(),
        email,
        created_by,
        created_at: now,
        expires_at: now + ttl,
        used_at: None,
        used_by: None,
    }
}

/// An unguessable invite token, 64 hex characters.
pub fn new_token() -> String {
    let mut s = Uuid::new_v4().simple().to_string();
    s.push_str(&Uuid::new_v4().simple().to_string());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_round_trip() {
        let hash = hash_password("correct horse battery").unwrap();
        assert!(hash.starts_with("$argon2"), "{hash}");
        assert!(verify_password("correct horse battery", &hash).unwrap());
        assert!(!verify_password("wrong horse battery", &hash).unwrap());
    }

    #[test]
    fn hashes_are_salted() {
        let a = hash_password("same password").unwrap();
        let b = hash_password("same password").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn unreadable_hashes_never_pass() {
        assert!(!verify_password("x", "not a phc string").unwrap());
        assert!(!verify_password("x", "").unwrap());
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(new_token(), new_token());
        assert_eq!(new_token().len(), 64);
    }

    #[test]
    fn invites_start_unused() {
        let now = Utc::now();
        let invite = new_invite(
            Uuid::new_v4(),
            Some("a@b.co".into()),
            Duration::from_secs(60),
            now,
        );
        assert!(invite.is_usable(now));
        assert_eq!(invite.expires_at, now + chrono::Duration::seconds(60));
        assert_eq!(invite.used_at, None);
    }

    #[test]
    fn login_url_keeps_the_destination() {
        assert_eq!(login_url("/items/new"), "/login?next=%2Fitems%2Fnew");
        assert_eq!(login_url(LOGIN_PATH), LOGIN_PATH);
        assert_eq!(login_url("https://evil.test"), LOGIN_PATH);
    }

    #[test]
    fn only_local_paths_are_followed() {
        assert_eq!(safe_next(Some("/items")).as_deref(), Some("/items"));
        assert_eq!(safe_next(Some("//evil.test")), None);
        assert_eq!(safe_next(Some("https://evil.test")), None);
        assert_eq!(safe_next(None), None);
    }
}
