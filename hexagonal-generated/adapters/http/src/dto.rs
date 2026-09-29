// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! What goes over the wire on `/v1`. Kept apart from the domain and
//! application types so the JSON can stay put while the model moves.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// An item as clients see it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
pub struct Item {
    pub id: Uuid,
    #[schema(example = "widget")]
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<domain::Item> for Item {
    fn from(item: domain::Item) -> Self {
        Self {
            id: item.id,
            name: item.name.into(),
            description: item.description,
            created_at: item.created_at,
            updated_at: item.updated_at,
        }
    }
}

/// Body of `POST /v1/items`; the domain checks `name`.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateItem {
    #[schema(example = "widget", max_length = 200)]
    pub name: String,
    pub description: Option<String>,
}

impl From<CreateItem> for application::CreateItem {
    fn from(input: CreateItem) -> Self {
        Self {
            name: input.name,
            description: input.description,
        }
    }
}

/// Body of `PUT /v1/items/{id}`; replaces every field.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateItem {
    #[schema(example = "widget", max_length = 200)]
    pub name: String,
    pub description: Option<String>,
}

impl From<UpdateItem> for application::UpdateItem {
    fn from(input: UpdateItem) -> Self {
        Self {
            name: input.name,
            description: input.description,
        }
    }
}

/// Query string of `GET /v1/items`.
#[derive(Clone, Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// Page size, 1 to 100, default 20.
    pub limit: Option<u32>,
    /// Number of items to skip, default 0.
    pub offset: Option<u32>,
}

impl From<ListQuery> for application::ListQuery {
    fn from(q: ListQuery) -> Self {
        let defaults = Self::default();
        Self {
            limit: q.limit.unwrap_or(defaults.limit),
            offset: q.offset.unwrap_or(defaults.offset),
        }
    }
}

/// One page of a listing.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Page {
    pub items: Vec<Item>,
    /// Total number of items, across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

impl From<application::Page> for Page {
    fn from(page: application::Page) -> Self {
        Self {
            items: page.items.into_iter().map(Item::from).collect(),
            total: page.total,
            limit: page.limit,
            offset: page.offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_query_defaults() {
        let q: application::ListQuery = ListQuery::default().into();
        assert_eq!(q, application::ListQuery::default());
        let q: application::ListQuery = ListQuery {
            limit: Some(5),
            offset: Some(7),
        }
        .into();
        assert_eq!((q.limit, q.offset), (5, 7));
    }
}
