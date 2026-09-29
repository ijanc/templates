// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! [`ItemRepository`] in a map: the store when nothing persistent is
//! wanted, and the test double of the core and of the HTTP adapter
//! otherwise. Contents are lost on exit.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use async_trait::async_trait;
use domain::{Item, ItemRepository, RepositoryError};
use uuid::Uuid;

type Items = HashMap<Uuid, Item>;

#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    items: Arc<RwLock<Items>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Items> {
        self.items.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Items> {
        self.items.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl ItemRepository for MemoryStore {
    async fn ping(&self) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn insert(&self, item: &Item) -> Result<(), RepositoryError> {
        let mut items = self.write();
        if items.contains_key(&item.id) {
            let msg = format!("duplicate id {}", item.id);
            return Err(RepositoryError::new(msg));
        }
        items.insert(item.id, item.clone());
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<Item>, RepositoryError> {
        Ok(self.read().get(&id).cloned())
    }

    async fn list(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Item>, RepositoryError> {
        let items = self.read();
        let mut all: Vec<&Item> = items.values().collect();
        all.sort_by_key(|i| (i.created_at, i.id));
        Ok(all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn count(&self) -> Result<u64, RepositoryError> {
        Ok(self.read().len() as u64)
    }

    async fn update(&self, item: &Item) -> Result<bool, RepositoryError> {
        match self.write().get_mut(&item.id) {
            Some(stored) => {
                *stored = item.clone();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError> {
        Ok(self.write().remove(&id).is_some())
    }
}
