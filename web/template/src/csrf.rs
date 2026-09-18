// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Cross-site request forgery protection: one token per session,
//! rendered into every form, checked on every unsafe method.

use axum::{
    body::Body,
    extract::Request,
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::{FORM_LIMIT, error::Error, session};

/// Session key holding the token.
pub const KEY: &str = "csrf";

/// Form field carrying the token back.
pub const FIELD: &str = "csrf_token";

/// A per session token; renders as its bare string.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CsrfToken(String);

impl CsrfToken {
    /// A fresh token, 64 hex characters.
    fn generate() -> Self {
        let mut s = Uuid::new_v4().simple().to_string();
        s.push_str(&Uuid::new_v4().simple().to_string());
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The token of `session`, creating and storing one on first use.
pub async fn token(session: &Session) -> Result<CsrfToken, Error> {
    if let Some(token) = session.get::<CsrfToken>(KEY).await? {
        return Ok(token);
    }
    let token = CsrfToken::generate();
    session.insert(KEY, &token).await?;
    Ok(token)
}

/// Reject unsafe requests whose form body does not carry the session
/// token. The body is buffered and put back, so handlers still read it.
pub async fn verify(req: Request, next: Next) -> Result<Response, Error> {
    let unsafe_method = matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if !unsafe_method {
        return Ok(next.run(req).await);
    }

    let session = session::get(&req)?;
    let expected = token(&session).await?;

    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, FORM_LIMIT)
        .await
        .map_err(|_| Error::PayloadTooLarge)?;
    let sent = serde_urlencoded::from_bytes::<Vec<(String, String)>>(&bytes)
        .ok()
        .and_then(|pairs| {
            pairs.into_iter().find(|(k, _)| k == FIELD).map(|(_, v)| v)
        });
    let ok = sent
        .as_deref()
        .is_some_and(|sent| equals(sent.as_bytes(), expected.as_str()));
    if !ok {
        return Err(Error::Forbidden("invalid or missing form token".into()));
    }

    Ok(next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await
        .into_response())
}

/// Compare in time independent of how far the values match.
fn equals(sent: &[u8], expected: &str) -> bool {
    let expected = expected.as_bytes();
    if sent.len() != expected.len() {
        return false;
    }
    sent.iter()
        .zip(expected)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_long() {
        let a = CsrfToken::generate();
        let b = CsrfToken::generate();
        assert_eq!(a.as_str().len(), 64);
        assert_ne!(a, b);
        assert!(a.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn serializes_as_a_bare_string() {
        let token = CsrfToken("abc".into());
        assert_eq!(serde_json::to_string(&token).unwrap(), "\"abc\"");
    }

    #[test]
    fn equality_is_exact() {
        assert!(equals(b"abc", "abc"));
        assert!(!equals(b"abc", "abd"));
        assert!(!equals(b"ab", "abc"));
        assert!(!equals(b"", "abc"));
    }
}
