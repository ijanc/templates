// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! In-memory store; contents are lost on exit.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use chrono::Utc;
use uuid::Uuid;
{% if auth == "none" %}
use crate::model::{CreateItem, Item, Page, UpdateItem};
{%- else %}
use crate::model::{CreateItem, Invite, Item, NewUser, Page, UpdateItem, User};
{%- endif %}

#[derive(Clone, Debug, Default)]
pub struct Store {
    items: Arc<RwLock<HashMap<Uuid, Item>>>,
{%- if auth != "none" %}
    users: Arc<RwLock<HashMap<Uuid, User>>>,
    invites: Arc<RwLock<HashMap<String, Invite>>>,
{%- endif %}
}

/// Reads and writes never poison the data, only the lock.
fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Always succeeds; exists for parity with the database stores.
    pub async fn ping(&self) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn create(&self, input: CreateItem) -> anyhow::Result<Item> {
        let now = Utc::now();
        let item = Item {
            id: Uuid::new_v4(),
            name: input.name,
            description: input.description,
            created_at: now,
            updated_at: now,
        };
        write(&self.items).insert(item.id, item.clone());
        Ok(item)
    }

    /// Oldest first; ties broken by id so pages are stable.
    pub async fn list(&self, limit: u32, offset: u32) -> anyhow::Result<Page> {
        let items = read(&self.items);
        let mut all: Vec<&Item> = items.values().collect();
        all.sort_by_key(|a| (a.created_at, a.id));
        let page = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(Page {
            items: page,
            total: items.len() as u64,
            limit,
            offset,
        })
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Item>> {
        Ok(read(&self.items).get(&id).cloned())
    }

    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateItem,
    ) -> anyhow::Result<Option<Item>> {
        let mut items = write(&self.items);
        let Some(item) = items.get_mut(&id) else {
            return Ok(None);
        };
        item.name = input.name;
        item.description = input.description;
        item.updated_at = Utc::now();
        Ok(Some(item.clone()))
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        Ok(write(&self.items).remove(&id).is_some())
    }
{%- if auth != "none" %}

    /// How many accounts exist; `0` enables the bootstrap invite.
    pub async fn user_count(&self) -> anyhow::Result<u64> {
        Ok(read(&self.users).len() as u64)
    }

    pub async fn user(&self, id: Uuid) -> anyhow::Result<Option<User>> {
        Ok(read(&self.users).get(&id).cloned())
    }

    /// Lookup by email, compared case insensitively.
    pub async fn user_by_email(
        &self,
        email: &str,
    ) -> anyhow::Result<Option<User>> {
        let email = email.to_lowercase();
        Ok(read(&self.users)
            .values()
            .find(|u| u.email == email)
            .cloned())
    }

    /// `None` when the email is already registered.
    pub async fn create_user(
        &self,
        input: NewUser,
    ) -> anyhow::Result<Option<User>> {
        let email = input.email.to_lowercase();
        let mut users = write(&self.users);
        if users.values().any(|u| u.email == email) {
            return Ok(None);
        }
        let now = Utc::now();
        let user = User {
            id: Uuid::new_v4(),
            email,
            name: input.name,
            avatar_url: input.avatar_url,
            password_hash: input.password_hash,
            provider_id: input.provider_id,
            created_at: now,
            updated_at: now,
        };
        users.insert(user.id, user.clone());
        Ok(Some(user))
    }

    pub async fn create_invite(
        &self,
        invite: Invite,
    ) -> anyhow::Result<Invite> {
        write(&self.invites).insert(invite.token.clone(), invite.clone());
        Ok(invite)
    }

    /// Newest first.
    pub async fn list_invites(&self) -> anyhow::Result<Vec<Invite>> {
        let invites = read(&self.invites);
        let mut all: Vec<Invite> = invites.values().cloned().collect();
        all.sort_by(|a, b| {
            b.created_at.cmp(&a.created_at).then(a.token.cmp(&b.token))
        });
        Ok(all)
    }

    pub async fn invite(&self, token: &str) -> anyhow::Result<Option<Invite>> {
        Ok(read(&self.invites).get(token).cloned())
    }

    /// Mark the invite used, if it still can be. `false` means someone
    /// else got there first, or it expired.
    pub async fn claim_invite(&self, token: &str) -> anyhow::Result<bool> {
        let now = Utc::now();
        let mut invites = write(&self.invites);
        let Some(invite) = invites.get_mut(token) else {
            return Ok(false);
        };
        if !invite.is_usable(now) {
            return Ok(false);
        }
        invite.used_at = Some(now);
        Ok(true)
    }

    /// Record who used a claimed invite, or release it when the account
    /// could not be created after all.
    pub async fn settle_invite(
        &self,
        token: &str,
        used_by: Option<Uuid>,
    ) -> anyhow::Result<()> {
        let mut invites = write(&self.invites);
        if let Some(invite) = invites.get_mut(token) {
            if used_by.is_none() {
                invite.used_at = None;
            }
            invite.used_by = used_by;
        }
        Ok(())
    }
{%- endif %}
}
