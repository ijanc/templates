// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The item pages: forms, redirects, flash messages and validation.

mod common;

use reqwest::StatusCode;

use crate::common::{TestServer, redirected_to};

/// Create an item through the form and return where it landed.
async fn create(s: &TestServer, name: &str) -> String {
    let res = s
        .post(
            "/items",
            "/items/new",
            &[("name", name), ("description", "made by a test")],
        )
        .await;
    redirected_to(&res)
}

#[tokio::test]
async fn create_redirects_and_flashes() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::sign_in(&s).await;

    let to = create(&s, "widget").await;
    assert!(to.starts_with("/items/"), "{to}");

    let html = s.html(&to).await;
    assert!(html.contains("item created"), "{html}");
    assert!(html.contains("widget"), "{html}");
    assert!(html.contains("made by a test"), "{html}");

    // A flash shows once and only once.
    let html = s.html(&to).await;
    assert!(!html.contains("item created"), "{html}");
}

#[tokio::test]
async fn created_items_are_listed() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::sign_in(&s).await;
    create(&s, "listed-item").await;

    let html = s.html("/items").await;
    assert!(html.contains("listed-item"), "{html}");
}

#[tokio::test]
async fn create_rejects_an_empty_name() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::sign_in(&s).await;

    let res = s.post("/items", "/items/new", &[("name", "  ")]).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = res.text().await.unwrap();
    assert!(html.contains("must not be blank"), "{html}");
    assert!(html.contains("is-invalid"), "{html}");

    let long = "x".repeat(201);
    let res = s.post("/items", "/items/new", &[("name", &long)]).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = res.text().await.unwrap();
    assert!(html.contains("at most 200 characters"), "{html}");
}

#[tokio::test]
async fn edit_replaces_the_item() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::sign_in(&s).await;
    let path = create(&s, "before").await;

    let edit = format!("{path}/edit");
    let html = s.html(&edit).await;
    assert!(html.contains("value=\"before\""), "{html}");

    let res = s.post(&path, &edit, &[("name", "after")]).await;
    assert_eq!(redirected_to(&res), path);

    let html = s.html(&path).await;
    assert!(html.contains("item saved"), "{html}");
    assert!(html.contains("after"), "{html}");
    assert!(!html.contains("made by a test"), "{html}");

    let res = s.post(&path, &edit, &[("name", "")]).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn delete_removes_the_item() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::sign_in(&s).await;
    let path = create(&s, "doomed").await;

    let delete = format!("{path}/delete");
    let res = s.post(&delete, &path, &[]).await;
    assert_eq!(redirected_to(&res), "/items");

    let html = s.html("/items").await;
    assert!(html.contains("item deleted"), "{html}");

    assert_eq!(s.get(&path).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        s.get(&delete).await.status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[tokio::test]
async fn unknown_items_are_404() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let path = format!("/items/{}", uuid::Uuid::new_v4());
    let res = s.get(&path).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = s.get("/items/not-a-uuid").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_list_is_paged() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::sign_in(&s).await;
    for i in 0..3 {
        create(&s, &format!("paged-{i}")).await;
    }

    let html = s.html("/items?limit=2").await;
    assert!(html.contains("offset=2"), "{html}");
    assert!(html.contains("of 3"), "{html}");

    let html = s.html("/items?limit=2&offset=2").await;
    assert!(html.contains("paged-2"), "{html}");
    assert!(!html.contains("paged-0"), "{html}");

    let res = s.get("/items?limit=0").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = s.get("/items?limit=nope").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
