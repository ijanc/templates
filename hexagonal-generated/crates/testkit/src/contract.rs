// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The checks behind `repository_contract_tests!`. Each one starts
//! from an empty repository.

use domain::{Item, ItemRepository, Name};
use tokio::sync::{Mutex, MutexGuard};
use uuid::Uuid;

static SERIAL: Mutex<()> = Mutex::const_new(());

/// Hold this for the whole test: a store shared between tests (a
/// database) is only safe one test at a time.
pub async fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().await
}

fn make(name: &str) -> Item {
    Item::new(Name::new(name).unwrap(), Some("d".into()))
}

pub async fn ping<R: ItemRepository>(repo: &R) {
    repo.ping().await.unwrap();
}

pub async fn insert_then_get<R: ItemRepository>(repo: &R) {
    let item = make("get");
    repo.insert(&item).await.unwrap();
    assert_eq!(repo.get(item.id).await.unwrap(), Some(item));
    assert_eq!(repo.get(Uuid::new_v4()).await.unwrap(), None);
}

pub async fn list_oldest_first<R: ItemRepository>(repo: &R) {
    let mut items = vec![make("a"), make("b"), make("c")];
    for item in &items {
        repo.insert(item).await.unwrap();
    }
    // Two items may share a timestamp; the id then decides.
    items.sort_by_key(|i| (i.created_at, i.id));
    assert_eq!(repo.list(10, 0).await.unwrap(), items);
    assert_eq!(repo.list(2, 1).await.unwrap(), items[1..].to_vec());
    assert!(repo.list(2, 3).await.unwrap().is_empty());
}

pub async fn count<R: ItemRepository>(repo: &R) {
    assert_eq!(repo.count().await.unwrap(), 0);
    repo.insert(&make("a")).await.unwrap();
    repo.insert(&make("b")).await.unwrap();
    assert_eq!(repo.count().await.unwrap(), 2);
}

pub async fn update_replaces<R: ItemRepository>(repo: &R) {
    let mut item = make("before");
    repo.insert(&item).await.unwrap();
    item.update(Name::new("after").unwrap(), None);
    assert!(repo.update(&item).await.unwrap());
    assert_eq!(repo.get(item.id).await.unwrap(), Some(item));
    assert!(!repo.update(&make("ghost")).await.unwrap());
}

pub async fn delete_twice<R: ItemRepository>(repo: &R) {
    let item = make("gone");
    repo.insert(&item).await.unwrap();
    assert!(repo.delete(item.id).await.unwrap());
    assert!(!repo.delete(item.id).await.unwrap());
    assert_eq!(repo.get(item.id).await.unwrap(), None);
}
