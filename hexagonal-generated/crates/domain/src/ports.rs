// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Driven ports: what the application needs from the outside world,
//! stated as traits. Adapters implement them; the application only
//! ever sees the trait.

use std::{error::Error, fmt};

use async_trait::async_trait;
use uuid::Uuid;

use crate::Item;

/// Persistence of items. `insert` and `update` take the whole entity:
/// the application builds it, the adapter only keeps it.
#[async_trait]
pub trait ItemRepository: Send + Sync {
    /// Readiness: the backing store answers.
    async fn ping(&self) -> Result<(), RepositoryError>;

    /// Store a new item; its id must not be in use.
    async fn insert(&self, item: &Item) -> Result<(), RepositoryError>;

    async fn get(&self, id: Uuid) -> Result<Option<Item>, RepositoryError>;

    /// Oldest first, ties broken by id, so pages are stable.
    async fn list(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Item>, RepositoryError>;

    /// Number of items, across all pages.
    async fn count(&self) -> Result<u64, RepositoryError>;

    /// Replace the stored item with the same id; `false` when there is
    /// none.
    async fn update(&self, item: &Item) -> Result<bool, RepositoryError>;

    /// `false` when there was nothing to delete.
    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError>;
}

/// An adapter failed. The cause is for the logs; clients get a generic
/// message. Displays and chains like the wrapped error.
#[derive(Debug)]
pub struct RepositoryError(Box<dyn Error + Send + Sync>);

impl RepositoryError {
    /// Wrap a cause: an error or a message.
    pub fn new(cause: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self(cause.into())
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error for RepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_reads_like_its_cause() {
        assert_eq!(RepositoryError::new("boom").to_string(), "boom");
        let io = std::io::Error::other("disk");
        assert_eq!(RepositoryError::new(io).to_string(), "disk");
    }
}
