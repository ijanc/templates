// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The repository contract over the in-memory store.

use store_memory::MemoryStore;

async fn store() -> Option<MemoryStore> {
    Some(MemoryStore::new())
}

testkit::repository_contract_tests!(store);
