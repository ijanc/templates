// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! One-shot messages carried across a redirect in the session.

use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::error::Error;

/// Session key holding the pending message.
pub const KEY: &str = "flash";

/// How a message is shown; maps to a Bootstrap alert colour.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Success,
    Error,
    Info,
}

impl Level {
    /// Name used by the alert class in the template.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Info => "info",
        }
    }
}

/// A message shown once, on the page the user is redirected to.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Flash {
    pub level: Level,
    pub message: String,
}

impl Flash {
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(Level::Success, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Level::Error, message)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Level::Info, message)
    }

    fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
        }
    }
}

/// Queue `flash` for the next request of this session.
pub async fn set(session: &Session, flash: Flash) -> Result<(), Error> {
    Ok(session.insert(KEY, flash).await?)
}

/// Take the pending message out of the session, if there is one.
pub async fn take(session: &Session) -> Result<Option<Flash>, Error> {
    Ok(session.remove::<Flash>(KEY).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_name_their_alert() {
        assert_eq!(Level::Success.as_str(), "success");
        assert_eq!(Flash::error("nope").level, Level::Error);
        assert_eq!(Flash::info("hi").message, "hi");
    }

    #[test]
    fn round_trips_through_json() {
        let flash = Flash::success("saved");
        let json = serde_json::to_string(&flash).unwrap();
        assert_eq!(json, r#"{"level":"success","message":"saved"}"#);
        let back: Flash = serde_json::from_str(&json).unwrap();
        assert_eq!(back, flash);
    }
}
