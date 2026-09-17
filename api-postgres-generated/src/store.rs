// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! PostgreSQL store through `sqlx`, schema in `migrations/`.

use chrono::Utc;
use sqlx::{PgPool as Pool, postgres::PgPoolOptions as PoolOptions};
use uuid::Uuid;

use crate::model::{CreateItem, Item, Page, UpdateItem};

#[derive(Clone, Debug)]
pub struct Store {
    pool: Pool,
}

impl Store {
    /// Open a connection pool to `url`.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let pool = PoolOptions::new().connect(url).await?;
        Ok(Self { pool })
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
        let item: Option<Item> = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at FROM items \
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
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
        Ok(item)
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let done = sqlx::query("DELETE FROM items WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }
}
