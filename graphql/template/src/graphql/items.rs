// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use async_graphql::{Context, Object};
use uuid::Uuid;
use validator::Validate;

use crate::{
    graphql::error::{Error, Result},
    model::{CreateItem, Item, ListQuery, Page, UpdateItem},
    store::Store,
};

fn store<'a>(ctx: &Context<'a>) -> &'a Store {
    ctx.data_unchecked::<Store>()
}

fn not_found(id: Uuid) -> Error {
    Error::NotFound(format!("item {id} not found"))
}

#[derive(Default)]
pub struct ItemQuery;

#[Object]
impl ItemQuery {
    /// One item; `null` when there is none with `id`.
    async fn item(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Item>> {
        Ok(store(ctx).get(id).await?)
    }

    /// A page of items, oldest first.
    async fn items(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Page size, 1 to 100, default 20")] limit: Option<u32>,
        #[graphql(desc = "Number of items to skip, default 0")] offset: Option<
            u32,
        >,
    ) -> Result<Page> {
        let q = ListQuery { limit, offset };
        q.validate()?;
        let (limit, offset) = q.limits();
        Ok(store(ctx).list(limit, offset).await?)
    }
}

#[derive(Default)]
pub struct ItemMutation;

#[Object]
impl ItemMutation {
    /// Create an item.
    async fn create_item(
        &self,
        ctx: &Context<'_>,
        input: CreateItem,
    ) -> Result<Item> {
        input.validate()?;
        Ok(store(ctx).create(input).await?)
    }

    /// Replace every field of an item.
    async fn update_item(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateItem,
    ) -> Result<Item> {
        input.validate()?;
        store(ctx)
            .update(id, input)
            .await?
            .ok_or_else(|| not_found(id))
    }

    /// Delete an item; returns its id.
    async fn delete_item(&self, ctx: &Context<'_>, id: Uuid) -> Result<Uuid> {
        if store(ctx).delete(id).await? {
            Ok(id)
        } else {
            Err(not_found(id))
        }
    }
}
