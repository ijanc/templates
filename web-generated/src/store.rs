// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! SQLite store through `sqlx`, schema in `migrations/`.

use chrono::Utc;
use sqlx::{SqlitePool as Pool, sqlite::SqlitePoolOptions as PoolOptions};
use uuid::Uuid;

use crate::model::{CreateItem, Invite, Item, NewUser, Page, UpdateItem, User};

#[derive(Clone, Debug)]
pub struct Store {
    pool: Pool,
}

impl Store {
    /// Open a connection pool to `url`.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
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
        Ok(Self { pool })
    }

    /// The pool, shared with the session store.
    pub fn pool(&self) -> &Pool {
        &self.pool
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
        let item = sqlx::query_as(
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

    /// How many accounts exist; `0` enables the bootstrap invite.
    pub async fn user_count(&self) -> anyhow::Result<u64> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(total.try_into()?)
    }

    pub async fn user(&self, id: Uuid) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as(&format!("{USER_COLUMNS} WHERE id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    /// Lookup by email, compared case insensitively.
    pub async fn user_by_email(
        &self,
        email: &str,
    ) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as(&format!("{USER_COLUMNS} WHERE email = $1"))
            .bind(email.to_lowercase())
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    /// `None` when the email is already registered.
    pub async fn create_user(
        &self,
        input: NewUser,
    ) -> anyhow::Result<Option<User>> {
        let now = Utc::now();
        let user = User {
            id: Uuid::new_v4(),
            email: input.email.to_lowercase(),
            name: input.name,
            avatar_url: input.avatar_url,
            password_hash: input.password_hash,
            provider_id: input.provider_id,
            created_at: now,
            updated_at: now,
        };
        let done = sqlx::query(
            "INSERT INTO users \
             (id, email, name, avatar_url, password_hash, provider_id, \
             created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(user.id)
        .bind(&user.email)
        .bind(&user.name)
        .bind(&user.avatar_url)
        .bind(&user.password_hash)
        .bind(&user.provider_id)
        .bind(user.created_at)
        .bind(user.updated_at)
        .execute(&self.pool)
        .await;
        match done {
            Ok(_) => Ok(Some(user)),
            Err(e) if is_unique_violation(&e) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_invite(
        &self,
        invite: Invite,
    ) -> anyhow::Result<Invite> {
        sqlx::query(
            "INSERT INTO invites \
             (token, email, created_by, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&invite.token)
        .bind(&invite.email)
        .bind(invite.created_by)
        .bind(invite.created_at)
        .bind(invite.expires_at)
        .execute(&self.pool)
        .await?;
        Ok(invite)
    }

    /// Newest first.
    pub async fn list_invites(&self) -> anyhow::Result<Vec<Invite>> {
        let invites = sqlx::query_as(&format!(
            "{INVITE_COLUMNS} ORDER BY created_at DESC, token"
        ))
        .fetch_all(&self.pool)
        .await?;
        Ok(invites)
    }

    pub async fn invite(&self, token: &str) -> anyhow::Result<Option<Invite>> {
        let invite =
            sqlx::query_as(&format!("{INVITE_COLUMNS} WHERE token = $1"))
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;
        Ok(invite)
    }

    /// Mark the invite used, if it still can be. `false` means someone
    /// else got there first, or it expired.
    pub async fn claim_invite(&self, token: &str) -> anyhow::Result<bool> {
        let now = Utc::now();
        let done = sqlx::query(
            "UPDATE invites SET used_at = $2 \
             WHERE token = $1 AND used_at IS NULL AND expires_at > $2",
        )
        .bind(token)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Record who used a claimed invite, or release it when the account
    /// could not be created after all.
    pub async fn settle_invite(
        &self,
        token: &str,
        used_by: Option<Uuid>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE invites SET used_by = $2, \
             used_at = CASE WHEN $3 THEN used_at ELSE NULL END \
             WHERE token = $1",
        )
        .bind(token)
        .bind(used_by)
        .bind(used_by.is_some())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Every column of `users`, in the order [`User`] declares them.
const USER_COLUMNS: &str = "SELECT id, email, name, avatar_url, \
     password_hash, provider_id, created_at, updated_at FROM users";

/// Every column of `invites`, in the order [`Invite`] declares them.
const INVITE_COLUMNS: &str = "SELECT token, email, created_by, created_at, \
     expires_at, used_at, used_by FROM invites";

/// A second account with the same email.
fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
}
