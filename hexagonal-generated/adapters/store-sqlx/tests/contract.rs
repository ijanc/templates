// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The repository contract over SQLite.

use store_sqlx::SqlxStore;

/// A migrated, empty store in memory.
async fn store() -> Option<SqlxStore> {
    let store = SqlxStore::connect("sqlite::memory:")
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
    Some(store)
}

testkit::repository_contract_tests!(store);
