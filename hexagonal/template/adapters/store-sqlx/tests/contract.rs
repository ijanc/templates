// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! The repository contract over {% if store == "postgres" %}PostgreSQL{% else %}SQLite{% endif %}.

use store_sqlx::SqlxStore;

{% if store == "postgres" -%}
/// A migrated, empty store on `DATABASE_URL`; `None` when unset. The
/// database is shared and the contract runs one test at a time, so
/// clearing the table here is enough.
{%- else -%}
/// A migrated, empty store in memory.
{%- endif %}
async fn store() -> Option<SqlxStore> {
{%- if store == "postgres" %}
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|u| !u.is_empty())?;
    let store = SqlxStore::connect(&url).await.expect("connect");
    store.migrate().await.expect("migrate");
    sqlx::query("DELETE FROM items")
        .execute(store.pool())
        .await
        .expect("clear");
{%- else %}
    let store = SqlxStore::connect("sqlite::memory:")
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
{%- endif %}
    Some(store)
}

testkit::repository_contract_tests!(store);
