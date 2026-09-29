// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! [`ItemUseCases`] over an [`ItemRepository`].

use std::sync::Arc;

use async_trait::async_trait;
use domain::{Item, ItemRepository, Name};
use uuid::Uuid;

use crate::{
    CreateItem, Error, FieldError, ItemUseCases, LIMIT_MAX, ListQuery, Page,
    UpdateItem,
};

/// The use cases; the store behind them is whatever implements the port.
#[derive(Clone)]
pub struct ItemService {
    repo: Arc<dyn ItemRepository>,
}

impl ItemService {
    pub fn new(repo: Arc<dyn ItemRepository>) -> Self {
        Self { repo }
    }
}

fn parse_name(name: String) -> Result<Name, Error> {
    Name::new(name).map_err(|e| Error::Invalid(vec![FieldError::name(e)]))
}

#[async_trait]
impl ItemUseCases for ItemService {
    async fn ready(&self) -> Result<(), Error> {
        Ok(self.repo.ping().await?)
    }

    async fn create(&self, input: CreateItem) -> Result<Item, Error> {
        let item = Item::new(parse_name(input.name)?, input.description);
        self.repo.insert(&item).await?;
        Ok(item)
    }

    async fn get(&self, id: Uuid) -> Result<Item, Error> {
        self.repo.get(id).await?.ok_or(Error::NotFound(id))
    }

    async fn list(&self, query: ListQuery) -> Result<Page, Error> {
        if !(1..=LIMIT_MAX).contains(&query.limit) {
            return Err(Error::Invalid(vec![FieldError::limit()]));
        }
        let items = self.repo.list(query.limit, query.offset).await?;
        let total = self.repo.count().await?;
        Ok(Page {
            items,
            total,
            limit: query.limit,
            offset: query.offset,
        })
    }

    async fn update(&self, id: Uuid, input: UpdateItem) -> Result<Item, Error> {
        let name = parse_name(input.name)?;
        let mut item = self.get(id).await?;
        item.update(name, input.description);
        if self.repo.update(&item).await? {
            Ok(item)
        } else {
            // Deleted between the read and the write.
            Err(Error::NotFound(id))
        }
    }

    async fn delete(&self, id: Uuid) -> Result<(), Error> {
        if self.repo.delete(id).await? {
            Ok(())
        } else {
            Err(Error::NotFound(id))
        }
    }
}
