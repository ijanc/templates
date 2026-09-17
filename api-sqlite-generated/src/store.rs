// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! SQLite store through `sqlx`, schema in `migrations/`.

use chrono::Utc;
use sqlx::{SqlitePool as Pool, sqlite::SqlitePoolOptions as PoolOptions};
use uuid::Uuid;

use crate::{
    cache::{self, Cache},
    model::{CreateItem, Item, Page, UpdateItem},
};

#[derive(Clone, Debug)]
pub struct Store {
    pool: Pool,
    /// Serves `get` when it has the item; `None` bypasses the cache.
    cache: Option<Cache>,
}

impl Store {
    /// Open a connection pool to `url`; reads go through a cache built
    /// from `cache`, none when `None`.
    pub async fn connect(
        url: &str,
        cache: Option<&cache::Config>,
    ) -> anyhow::Result<Self> {
        let mut opts = PoolOptions::new();
        if url.contains(":memory:") {
            // Every connection gets its own in-memory database; keep a
            // single one open for the whole lifetime of the pool.
            opts = opts
                .max_connections(1)
                .idle_timeout(None)
                .max_lifetime(None);
        }
        let pool = opts.connect(url).await?;
        let cache = cache.map(Cache::new);
        Ok(Self { pool, cache })
    }

    /// Apply pending migrations embedded from `migrations/`.
    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!().run(&self.pool).await?;
        Ok(())
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
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
        sqlx::query(
            "INSERT INTO items (id, name, description, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(item.id)
        .bind(&item.name)
        .bind(&item.description)
        .bind(item.created_at)
        .bind(item.updated_at)
        .execute(&self.pool)
        .await?;
        if let Some(c) = &self.cache {
            c.insert(&item).await;
        }
        Ok(item)
    }

    /// Oldest first; ties broken by id so pages are stable.
    pub async fn list(&self, limit: u32, offset: u32) -> anyhow::Result<Page> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(&self.pool)
            .await?;
        let items = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at FROM items \
             ORDER BY created_at, id LIMIT $1 OFFSET $2",
        )
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&self.pool)
        .await?;
        Ok(Page {
            items,
            total: total.try_into()?,
            limit,
            offset,
        })
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Item>> {
        if let Some(c) = &self.cache
            && let Some(item) = c.get(id).await
        {
            return Ok(Some(item));
        }
        let item: Option<Item> = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at FROM items \
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        if let (Some(c), Some(item)) = (&self.cache, &item) {
            c.insert(item).await;
        }
        Ok(item)
    }

    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateItem,
    ) -> anyhow::Result<Option<Item>> {
        let item = sqlx::query_as(
            "UPDATE items SET name = $2, description = $3, updated_at = $4 \
             WHERE id = $1 \
             RETURNING id, name, description, created_at, updated_at",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(Utc::now())
        .fetch_optional(&self.pool)
        .await?;
        if let Some(c) = &self.cache {
            match &item {
                Some(item) => c.insert(item).await,
                None => c.invalidate(id).await,
            }
        }
        Ok(item)
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let done = sqlx::query("DELETE FROM items WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if let Some(c) = &self.cache {
            c.invalidate(id).await;
        }
        Ok(done.rows_affected() > 0)
    }
}
