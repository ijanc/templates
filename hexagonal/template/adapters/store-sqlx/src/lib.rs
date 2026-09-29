// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! [`ItemRepository`] on {% if store == "postgres" %}PostgreSQL{% else %}SQLite{% endif %} through `sqlx`; schema in
//! `migrations/`. Queries are plain strings with bind parameters, so
//! no `DATABASE_URL` is needed at build time.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{Item, ItemRepository, Name, RepositoryError};
{%- if store == "postgres" %}
use sqlx::{PgPool as Pool, postgres::PgPoolOptions as PoolOptions};
{%- else %}
use sqlx::{SqlitePool as Pool, sqlite::SqlitePoolOptions as PoolOptions};
{%- endif %}
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct SqlxStore {
    pool: Pool,
}

impl SqlxStore {
    /// Open a connection pool to `url`.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
{%- if store == "sqlite" %}
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
{%- else %}
        let pool = PoolOptions::new().connect(url).await?;
{%- endif %}
        Ok(Self { pool })
    }

    /// Apply pending migrations embedded from `migrations/`.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!().run(&self.pool).await
    }

    /// The pool, for anything else that shares the database.
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
}

/// One row of `items`. Rows go through the domain constructors, so
/// stored data still meets the invariants.
#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    name: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<Row> for Item {
    type Error = RepositoryError;

    fn try_from(row: Row) -> Result<Self, RepositoryError> {
        let name = Name::new(row.name).map_err(|e| {
            RepositoryError::new(format!("item {}: name {e}", row.id))
        })?;
        Ok(Item {
            id: row.id,
            name,
            description: row.description,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[async_trait]
impl ItemRepository for SqlxStore {
    async fn ping(&self) -> Result<(), RepositoryError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::new)?;
        Ok(())
    }

    async fn insert(&self, item: &Item) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO items (id, name, description, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(item.id)
        .bind(item.name.as_str())
        .bind(&item.description)
        .bind(item.created_at)
        .bind(item.updated_at)
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::new)?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<Item>, RepositoryError> {
        let row: Option<Row> = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at FROM items \
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::new)?;
        row.map(Item::try_from).transpose()
    }

    async fn list(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Item>, RepositoryError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at FROM items \
             ORDER BY created_at, id LIMIT $1 OFFSET $2",
        )
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::new)?;
        rows.into_iter().map(Item::try_from).collect()
    }

    async fn count(&self) -> Result<u64, RepositoryError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::new)?;
        u64::try_from(n).map_err(RepositoryError::new)
    }

    async fn update(&self, item: &Item) -> Result<bool, RepositoryError> {
        let done = sqlx::query(
            "UPDATE items SET name = $2, description = $3, updated_at = $4 \
             WHERE id = $1",
        )
        .bind(item.id)
        .bind(item.name.as_str())
        .bind(&item.description)
        .bind(item.updated_at)
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::new)?;
        Ok(done.rows_affected() > 0)
    }

    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let done = sqlx::query("DELETE FROM items WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::new)?;
        Ok(done.rows_affected() > 0)
    }
}
