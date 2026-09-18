// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError, ValidationErrors};

/// Longest accepted `name`.
pub const NAME_MAX: u64 = 200;

/// Default and maximum page size.
pub const LIMIT_DEFAULT: u32 = 20;
pub const LIMIT_MAX: u32 = 100;

/// Shortest accepted password.
pub const PASSWORD_MIN: u64 = 12;

/// A stored item.
#[derive(
    Clone, Debug, Deserialize, PartialEq, Eq, Serialize, sqlx::FromRow,
)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Body of `POST /items`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Validate)]
pub struct CreateItem {
    #[validate(
        custom(function = "not_blank"),
        length(max = NAME_MAX, message = "must be at most 200 characters")
    )]
    pub name: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub description: Option<String>,
}

/// Body of `POST /items/{id}`; replaces every field.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Validate)]
pub struct UpdateItem {
    #[validate(
        custom(function = "not_blank"),
        length(max = NAME_MAX, message = "must be at most 200 characters")
    )]
    pub name: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub description: Option<String>,
}

impl From<&Item> for UpdateItem {
    fn from(item: &Item) -> Self {
        Self {
            name: item.name.clone(),
            description: item.description.clone(),
        }
    }
}

/// Rejects empty and whitespace-only strings.
fn not_blank(s: &str) -> Result<(), ValidationError> {
    if s.trim().is_empty() {
        return Err(ValidationError::new("blank")
            .with_message("must not be blank".into()));
    }
    Ok(())
}

/// An omitted or empty form field is no value at all.
fn empty_as_none<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(d)?;
    Ok(value.filter(|s| !s.trim().is_empty()))
}

/// Failed checks per field, ready to render next to the inputs.
pub fn field_errors(e: &ValidationErrors) -> BTreeMap<String, Vec<String>> {
    e.field_errors()
        .into_iter()
        .map(|(field, errors)| {
            let messages = errors
                .iter()
                .map(|e| e.message.as_deref().unwrap_or(&e.code).to_string())
                .collect();
            (field.to_string(), messages)
        })
        .collect()
}

/// Query string of `GET /items`.
#[derive(Clone, Debug, Default, Deserialize, Validate)]
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
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    /// Total number of items, across all pages.
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

impl Page {
    /// Offset of the previous page, or `None` on the first one.
    pub fn prev(&self) -> Option<u32> {
        (self.offset > 0).then(|| self.offset.saturating_sub(self.limit))
    }

    /// Offset of the next page, or `None` on the last one.
    pub fn next(&self) -> Option<u32> {
        let next = self.offset.saturating_add(self.limit);
        (u64::from(next) < self.total).then_some(next)
    }
}

/// An account. `password_hash` and `provider_id` never reach a
/// template.
#[derive(
    Clone, Debug, Deserialize, PartialEq, Eq, Serialize, sqlx::FromRow,
)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    #[serde(skip_serializing)]
    pub provider_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An account about to be stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUser {
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub password_hash: Option<String>,
    pub provider_id: Option<String>,
}

/// A single use registration token.
#[derive(
    Clone, Debug, Deserialize, PartialEq, Eq, Serialize, sqlx::FromRow,
)]
pub struct Invite {
    pub token: String,
    pub email: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub used_by: Option<Uuid>,
}

impl Invite {
    /// Unused and not yet expired at `now`.
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        self.used_at.is_none() && self.expires_at > now
    }
}

/// Body of `POST /login`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Validate)]
pub struct LoginForm {
    #[validate(email(message = "must be an email address"))]
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
    /// Where to go once logged in; only in-app paths are honoured.
    #[serde(default, deserialize_with = "empty_as_none")]
    pub next: Option<String>,
}

/// Body of `POST /register`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Validate)]
pub struct RegisterForm {
    #[validate(custom(function = "not_blank"))]
    pub token: String,
    #[validate(email(message = "must be an email address"))]
    pub email: String,
    #[validate(
        custom(function = "not_blank"),
        length(max = NAME_MAX, message = "must be at most 200 characters")
    )]
    pub name: String,
    #[serde(skip_serializing)]
    #[validate(length(
        min = PASSWORD_MIN,
        message = "must be at least 12 characters"
    ))]
    pub password: String,
}

/// Body of `POST /invites`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Validate)]
pub struct InviteForm {
    /// Who the invite is meant for; a reminder, not a restriction.
    #[serde(default, deserialize_with = "empty_as_none")]
    #[validate(email(message = "must be an email address"))]
    pub email: Option<String>,
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

    fn codes(r: Result<(), ValidationErrors>) -> Vec<String> {
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
    }

    #[test]
    fn messages_are_grouped_by_field() {
        let e = item("").validate().unwrap_err();
        let errors = field_errors(&e);
        assert_eq!(errors["name"], ["must not be blank"]);
    }

    #[test]
    fn blank_description_is_no_description() {
        let form = "name=x&description=";
        let c: CreateItem = serde_urlencoded::from_str(form).unwrap();
        assert_eq!(c.description, None);
        let form = "name=x&description=%20%20";
        let c: CreateItem = serde_urlencoded::from_str(form).unwrap();
        assert_eq!(c.description, None);
        let form = "name=x&description=d";
        let c: CreateItem = serde_urlencoded::from_str(form).unwrap();
        assert_eq!(c.description.as_deref(), Some("d"));
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

    #[test]
    fn page_links() {
        let page = |total, limit, offset| Page {
            items: Vec::new(),
            total,
            limit,
            offset,
        };
        assert_eq!(page(5, 2, 0).prev(), None);
        assert_eq!(page(5, 2, 0).next(), Some(2));
        assert_eq!(page(5, 2, 2).prev(), Some(0));
        assert_eq!(page(5, 2, 4).next(), None);
        assert_eq!(page(0, 2, 0).next(), None);
    }

    #[test]
    fn password_length_is_enforced() {
        let form = |password: &str| RegisterForm {
            token: "t".into(),
            email: "a@b.co".into(),
            name: "A".into(),
            password: password.into(),
        };
        assert!(form(&"x".repeat(PASSWORD_MIN as usize)).validate().is_ok());
        let short = form(&"x".repeat(PASSWORD_MIN as usize - 1));
        assert_eq!(codes(short.validate()), ["password:length"]);
        let bad_email = RegisterForm {
            email: "nope".into(),
            ..form(&"x".repeat(PASSWORD_MIN as usize))
        };
        assert_eq!(codes(bad_email.validate()), ["email:email"]);
    }

    #[test]
    fn invites_expire_and_are_single_use() {
        let now = Utc::now();
        let mut invite = Invite {
            token: "t".into(),
            email: None,
            created_by: Uuid::new_v4(),
            created_at: now,
            expires_at: now + chrono::Duration::hours(1),
            used_at: None,
            used_by: None,
        };
        assert!(invite.is_usable(now));
        invite.used_at = Some(now);
        assert!(!invite.is_usable(now));
        invite.used_at = None;
        invite.expires_at = now - chrono::Duration::hours(1);
        assert!(!invite.is_usable(now));
    }

    #[test]
    fn secrets_are_not_serialized() {
        let user = User {
            id: Uuid::new_v4(),
            email: "a@b.co".into(),
            name: "A".into(),
            avatar_url: None,
            password_hash: Some("$argon2id$secret".into()),
            provider_id: Some("1234".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(!json.contains("secret"), "{json}");
        assert!(!json.contains("1234"), "{json}");
    }
}
