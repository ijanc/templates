// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The driving port and its inputs and outputs.

use async_trait::async_trait;
use domain::Item;
use uuid::Uuid;

use crate::Error;

/// Page size when the caller gives none.
pub const LIMIT_DEFAULT: u32 = 20;

/// Largest accepted page size.
pub const LIMIT_MAX: u32 = 100;

/// Input of [`ItemUseCases::create`]; `name` is checked by the domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateItem {
    pub name: String,
    pub description: Option<String>,
}

/// Input of [`ItemUseCases::update`]; replaces every field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateItem {
    pub name: String,
    pub description: Option<String>,
}

/// Input of [`ItemUseCases::list`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListQuery {
    /// Page size, 1 to [`LIMIT_MAX`].
    pub limit: u32,
    /// Number of items to skip.
    pub offset: u32,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            limit: LIMIT_DEFAULT,
            offset: 0,
        }
    }
}

/// One page of a listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub items: Vec<Item>,
    /// Number of items across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// Everything a driving adapter can ask for. Failures are [`Error`]s;
/// each adapter translates them into its own idiom.
#[async_trait]
pub trait ItemUseCases: Send + Sync {
    /// Readiness: the repository answers.
    async fn ready(&self) -> Result<(), Error>;

    async fn create(&self, input: CreateItem) -> Result<Item, Error>;

    async fn get(&self, id: Uuid) -> Result<Item, Error>;

    async fn list(&self, query: ListQuery) -> Result<Page, Error>;

    async fn update(&self, id: Uuid, input: UpdateItem) -> Result<Item, Error>;

    async fn delete(&self, id: Uuid) -> Result<(), Error>;
}
