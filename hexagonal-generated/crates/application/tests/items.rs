// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The use cases over the in-memory store.

use std::{fmt::Debug, sync::Arc};

use application::{
    CreateItem, Error, ItemService, ItemUseCases, LIMIT_MAX, ListQuery,
    UpdateItem,
};
use store_memory::MemoryStore;
use uuid::Uuid;

fn service() -> ItemService {
    ItemService::new(Arc::new(MemoryStore::new()))
}

fn input(name: &str) -> CreateItem {
    CreateItem {
        name: name.into(),
        description: Some("d".into()),
    }
}

/// `(field, code)` of every entry of an `Error::Invalid`.
fn invalid<T: Debug>(r: Result<T, Error>) -> Vec<(&'static str, &'static str)> {
    match r {
        Err(Error::Invalid(errors)) => {
            errors.iter().map(|e| (e.field, e.code)).collect()
        }
        other => panic!("expected invalid input, got {other:?}"),
    }
}

#[tokio::test]
async fn ready() {
    service().ready().await.unwrap();
}

#[tokio::test]
async fn create_checks_the_name() {
    let s = service();
    assert_eq!(invalid(s.create(input("")).await), [("name", "blank")]);
    let long = "x".repeat(201);
    assert_eq!(invalid(s.create(input(&long)).await), [("name", "length")]);
    let item = s.create(input("widget")).await.unwrap();
    assert_eq!(item.name.as_str(), "widget");
    assert_eq!(item.description.as_deref(), Some("d"));
}

#[tokio::test]
async fn get_found_and_missing() {
    let s = service();
    let item = s.create(input("get")).await.unwrap();
    assert_eq!(s.get(item.id).await.unwrap(), item);
    let id = Uuid::new_v4();
    assert!(matches!(s.get(id).await, Err(Error::NotFound(got)) if got == id));
}

#[tokio::test]
async fn list_pages_and_bounds() {
    let s = service();
    for i in 0..3 {
        s.create(input(&format!("list-{i}"))).await.unwrap();
    }
    let page = s
        .list(ListQuery {
            limit: 2,
            offset: 0,
        })
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!((page.total, page.limit, page.offset), (3, 2, 0));
    let rest = s
        .list(ListQuery {
            limit: 2,
            offset: 2,
        })
        .await
        .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert!(!page.items.contains(&rest.items[0]));
    let all = s.list(ListQuery::default()).await.unwrap();
    assert_eq!(all.items.len(), 3);
    for limit in [0, LIMIT_MAX + 1] {
        let r = s.list(ListQuery { limit, offset: 0 }).await;
        assert_eq!(invalid(r), [("limit", "range")]);
    }
}

#[tokio::test]
async fn update_replaces_and_checks() {
    let s = service();
    let item = s.create(input("before")).await.unwrap();
    let after = UpdateItem {
        name: "after".into(),
        description: None,
    };
    let updated = s.update(item.id, after).await.unwrap();
    assert_eq!(updated.id, item.id);
    assert_eq!(updated.name.as_str(), "after");
    assert_eq!(updated.description, None);
    assert_eq!(updated.created_at, item.created_at);
    assert!(updated.updated_at >= item.updated_at);
    assert_eq!(s.get(item.id).await.unwrap(), updated);

    let blank = UpdateItem {
        name: String::new(),
        description: None,
    };
    assert_eq!(invalid(s.update(item.id, blank).await), [("name", "blank")]);
    let ok = UpdateItem {
        name: "x".into(),
        description: None,
    };
    let r = s.update(Uuid::new_v4(), ok).await;
    assert!(matches!(r, Err(Error::NotFound(_))), "{r:?}");
}

#[tokio::test]
async fn delete_then_not_found() {
    let s = service();
    let item = s.create(input("gone")).await.unwrap();
    s.delete(item.id).await.unwrap();
    let r = s.delete(item.id).await;
    assert!(matches!(r, Err(Error::NotFound(_))), "{r:?}");
    let r = s.get(item.id).await;
    assert!(matches!(r, Err(Error::NotFound(_))), "{r:?}");
}
