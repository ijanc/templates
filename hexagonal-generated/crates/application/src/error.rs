// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! What a use case can fail with.

use std::fmt;

use domain::{NameError, RepositoryError};
use uuid::Uuid;

/// One rejected input field; `code` is stable per check, `message` is
/// for people.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldError {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

impl FieldError {
    /// `name` broke a domain rule.
    pub fn name(e: NameError) -> Self {
        let code = match e {
            NameError::Blank => "blank",
            NameError::TooLong => "length",
        };
        Self {
            field: "name",
            code,
            message: e.to_string(),
        }
    }

    /// `limit` is out of range.
    pub fn limit() -> Self {
        Self {
            field: "limit",
            code: "range",
            message: format!("must be 1 to {}", crate::LIMIT_MAX),
        }
    }
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("item {0} not found")]
    NotFound(Uuid),
    /// The input broke a rule; one entry per failed check.
    #[error("invalid input: {}", join(.0))]
    Invalid(Vec<FieldError>),
    /// The repository failed; the cause is logged, not shown.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn join(errors: &[FieldError]) -> String {
    let parts: Vec<String> = errors.iter().map(ToString::to_string).collect();
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages() {
        let id = Uuid::nil();
        assert_eq!(
            Error::NotFound(id).to_string(),
            format!("item {id} not found")
        );
        let e = Error::Invalid(vec![
            FieldError::name(NameError::Blank),
            FieldError::limit(),
        ]);
        assert_eq!(
            e.to_string(),
            "invalid input: name: must not be blank, limit: must be 1 to 100"
        );
        let e = Error::from(RepositoryError::new("boom"));
        assert_eq!(e.to_string(), "boom");
    }
}
