// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! In-memory store; contents are lost on exit.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use chrono::Utc;
use uuid::Uuid;

use crate::model::{CreateItem, Item, Page, UpdateItem};

#[derive(Clone, Debug, Default)]
pub struct Store {
    items: Arc<RwLock<HashMap<Uuid, Item>>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<Uuid, Item>> {
        self.items.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<Uuid, Item>> {
        self.items.write().unwrap_or_else(|e| e.into_inner())
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
        self.write().insert(item.id, item.clone());
        Ok(item)
    }

    /// Oldest first; ties broken by id so pages are stable.
    pub async fn list(&self, limit: u32, offset: u32) -> anyhow::Result<Page> {
        let items = self.read();
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
        Ok(self.read().get(&id).cloned())
    }

    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateItem,
    ) -> anyhow::Result<Option<Item>> {
        let mut items = self.write();
        let Some(item) = items.get_mut(&id) else {
            return Ok(None);
        };
        item.name = input.name;
        item.description = input.description;
        item.updated_at = Utc::now();
        Ok(Some(item.clone()))
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        Ok(self.write().remove(&id).is_some())
    }
}
