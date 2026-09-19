// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

/// Errors returned by this crate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The name was empty or whitespace only.
    #[error("name is empty")]
    Empty,
}

/// Result alias using [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
