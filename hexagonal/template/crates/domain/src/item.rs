// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! The item entity and the name it carries.

use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Longest accepted name, in characters.
pub const NAME_MAX: usize = 200;

/// A name that is not blank and at most [`NAME_MAX`] characters long.
/// [`Name::new`] is the only way to get one, so an [`Item`] never holds
/// an invalid name and nothing downstream needs to check again.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(String);

/// Why a string is not a [`Name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("must not be blank")]
    Blank,
    #[error("must be at most {NAME_MAX} characters")]
    TooLong,
}

impl Name {
    pub fn new(name: impl Into<String>) -> Result<Self, NameError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(NameError::Blank);
        }
        if name.chars().count() > NAME_MAX {
            return Err(NameError::TooLong);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Name> for String {
    fn from(name: Name) -> Self {
        name.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A stored item. Timestamps have microsecond precision, the coarsest
/// any store keeps, so an item reads back equal to the one written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Item {
    /// A new item with a random id, created now.
    pub fn new(name: Name, description: Option<String>) -> Self {
        let now = now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            created_at: now,
            updated_at: now,
        }
    }

    /// Replace every editable field and bump `updated_at`.
    pub fn update(&mut self, name: Name, description: Option<String>) {
        self.name = name;
        self.description = description;
        self.updated_at = now();
    }
}

/// The current time, truncated to microseconds.
fn now() -> DateTime<Utc> {
    let t = Utc::now();
    DateTime::from_timestamp_micros(t.timestamp_micros()).unwrap_or(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_bounds() {
        assert!(Name::new("x").is_ok());
        assert!(Name::new("x".repeat(NAME_MAX)).is_ok());
        assert_eq!(Name::new(""), Err(NameError::Blank));
        assert_eq!(Name::new("  "), Err(NameError::Blank));
        let long = "x".repeat(NAME_MAX + 1);
        assert_eq!(Name::new(long), Err(NameError::TooLong));
        // Characters, not bytes.
        assert!(Name::new("é".repeat(NAME_MAX)).is_ok());
    }

    #[test]
    fn name_keeps_its_text() {
        let name = Name::new(" widget ").unwrap();
        assert_eq!(name.as_str(), " widget ");
        assert_eq!(name.to_string(), " widget ");
        assert_eq!(String::from(name), " widget ");
    }

    #[test]
    fn new_item_is_fresh() {
        let item = Item::new(Name::new("x").unwrap(), None);
        assert_eq!(item.created_at, item.updated_at);
        assert_eq!(item.created_at.timestamp_subsec_nanos() % 1_000, 0);
        let other = Item::new(Name::new("x").unwrap(), None);
        assert_ne!(item.id, other.id);
    }

    #[test]
    fn update_bumps_updated_at() {
        let mut item = Item::new(Name::new("before").unwrap(), None);
        let (id, created_at) = (item.id, item.created_at);
        item.update(Name::new("after").unwrap(), Some("d".into()));
        assert_eq!(item.id, id);
        assert_eq!(item.created_at, created_at);
        assert!(item.updated_at >= created_at);
        assert_eq!(item.name.as_str(), "after");
        assert_eq!(item.description.as_deref(), Some("d"));
    }
}
