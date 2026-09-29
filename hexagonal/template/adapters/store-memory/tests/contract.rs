// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! The repository contract over the in-memory store.

use store_memory::MemoryStore;

async fn store() -> Option<MemoryStore> {
    Some(MemoryStore::new())
}

testkit::repository_contract_tests!(store);
