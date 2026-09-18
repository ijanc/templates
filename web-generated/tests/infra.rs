// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Probes, request ids, static files, error pages and the form token.

mod common;

use reqwest::StatusCode;
use uuid::Uuid;

use crate::common::{Health, redirected_to};

#[tokio::test]
async fn healthz() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/healthz").await;
    assert_eq!(res.status(), StatusCode::OK);
    let h: Health = res.json().await.unwrap();
    assert_eq!(h.status, "ok");
    assert!(
        h.version.starts_with(env!("CARGO_PKG_VERSION")),
        "{}",
        h.version
    );
}

#[tokio::test]
async fn readyz() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/readyz").await;
    assert_eq!(res.status(), StatusCode::OK);
    let h: Health = res.json().await.unwrap();
    assert_eq!(h.status, "ok");
}

#[tokio::test]
async fn root_goes_to_items() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/").await;
    assert_eq!(redirected_to(&res), "/items");
}

#[tokio::test]
async fn request_id_generated() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/healthz").await;
    let id = res.headers()["x-request-id"].to_str().unwrap();
    let id = Uuid::parse_str(id).unwrap();
    assert_eq!(id.get_version(), Some(uuid::Version::Random));
}

#[tokio::test]
async fn request_id_echoed() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .get(s.url("/healthz"))
        .header("x-request-id", "abc-123")
        .send()
        .await
        .unwrap();
    assert_eq!(res.headers()["x-request-id"], "abc-123");
}

#[tokio::test]
async fn static_files_are_served() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/static/robots.txt").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.text().await.unwrap().contains("User-agent"));
}

#[tokio::test]
async fn pages_render_without_built_css() {
    let Some(s) = common::spawn().await else {
        return;
    };
    // `npm run build` has not run, so the stylesheet is missing; the
    // page must still be fine.
    let res = s.get("/static/css/app.css").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let html = s.html("/items").await;
    assert!(html.contains("/static/css/app.css"), "{html}");
}

#[tokio::test]
async fn unknown_path_renders_the_404_page() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/nope").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let ctype = res.headers()["content-type"].to_str().unwrap().to_owned();
    assert!(ctype.starts_with("text/html"), "{ctype}");
    let html = res.text().await.unwrap();
    assert!(html.contains("Page not found"), "{html}");
    assert!(html.contains("nope"), "{html}");
    assert!(html.contains("<!doctype html>"), "{html}");
}

#[tokio::test]
async fn method_not_allowed_renders_a_page() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let path = format!("/items/{}/delete", Uuid::new_v4());
    let res = s.get(&path).await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    let html = res.text().await.unwrap();
    assert!(html.contains("405"), "{html}");
    assert!(html.contains("<!doctype html>"), "{html}");
}

#[tokio::test]
async fn posts_without_a_token_are_refused() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.post_raw("/items", &[("name", "sneaky")]).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let html = res.text().await.unwrap();
    assert!(html.contains("403"), "{html}");

    let res = s
        .post_raw("/items", &[("name", "sneaky"), ("csrf_token", "wrong")])
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
