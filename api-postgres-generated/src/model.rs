// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Longest accepted `name`.
pub const NAME_MAX: u64 = 200;

/// Default and maximum page size.
pub const LIMIT_DEFAULT: u32 = 20;
pub const LIMIT_MAX: u32 = 100;

/// A stored item.
#[derive(
    Clone, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema, sqlx::FromRow,
)]
pub struct Item {
    pub id: Uuid,
    #[schema(example = "widget")]
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Body of `POST /v1/items`.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct CreateItem {
    #[schema(example = "widget", max_length = 200)]
    #[validate(
        custom(function = "not_blank"),
        length(max = NAME_MAX, message = "must be at most 200 characters")
    )]
    pub name: String,
    pub description: Option<String>,
}

/// Body of `PUT /v1/items/{id}`; replaces every field.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct UpdateItem {
    #[schema(example = "widget", max_length = 200)]
    #[validate(
        custom(function = "not_blank"),
        length(max = NAME_MAX, message = "must be at most 200 characters")
    )]
    pub name: String,
    pub description: Option<String>,
}

/// Rejects empty and whitespace-only strings.
fn not_blank(s: &str) -> Result<(), ValidationError> {
    if s.trim().is_empty() {
        return Err(ValidationError::new("blank")
            .with_message("must not be blank".into()));
    }
    Ok(())
}

/// Query string of `GET /v1/items`.
#[derive(Clone, Debug, Default, Deserialize, IntoParams, Validate)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// Page size, 1 to 100, default 20.
    #[validate(range(min = 1, max = LIMIT_MAX, message = "must be 1 to 100"))]
    pub limit: Option<u32>,
    /// Number of items to skip, default 0.
    pub offset: Option<u32>,
}

impl ListQuery {
    /// `(limit, offset)` with defaults applied.
    pub fn limits(&self) -> (u32, u32) {
        (
            self.limit.unwrap_or(LIMIT_DEFAULT),
            self.offset.unwrap_or(0),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> CreateItem {
        CreateItem {
            name: name.into(),
            description: None,
        }
    }

    fn codes(r: Result<(), validator::ValidationErrors>) -> Vec<String> {
        let e = r.unwrap_err();
        let mut v: Vec<String> = e
            .field_errors()
            .into_iter()
            .flat_map(|(f, es)| {
                es.iter().map(move |e| format!("{f}:{}", e.code))
            })
            .collect();
        v.sort();
        v
    }

    #[test]
    fn name_bounds() {
        assert!(item("x").validate().is_ok());
        assert!(item(&"x".repeat(NAME_MAX as usize)).validate().is_ok());
        assert_eq!(codes(item("").validate()), ["name:blank"]);
        assert_eq!(codes(item("  ").validate()), ["name:blank"]);
        assert_eq!(
            codes(item(&"x".repeat(NAME_MAX as usize + 1)).validate()),
            ["name:length"]
        );
        let u = UpdateItem {
            name: String::new(),
            description: None,
        };
        assert_eq!(codes(u.validate()), ["name:blank"]);
    }

    #[test]
    fn list_bounds() {
        let q = ListQuery::default();
        assert!(q.validate().is_ok());
        assert_eq!(q.limits(), (LIMIT_DEFAULT, 0));
        let q = ListQuery {
            limit: Some(0),
            offset: None,
        };
        assert_eq!(codes(q.validate()), ["limit:range"]);
        let q = ListQuery {
            limit: Some(LIMIT_MAX + 1),
            offset: None,
        };
        assert_eq!(codes(q.validate()), ["limit:range"]);
        let q = ListQuery {
            limit: Some(LIMIT_MAX),
            offset: Some(7),
        };
        assert!(q.validate().is_ok());
        assert_eq!(q.limits(), (LIMIT_MAX, 7));
    }
}
