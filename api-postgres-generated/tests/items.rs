// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! `/v1/items` CRUD over HTTP.

mod common;

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::{Item, Page, Problem, TestServer};

async fn create(s: &TestServer, name: &str) -> Item {
    create_with_etag(s, name).await.0
}

async fn create_with_etag(s: &TestServer, name: &str) -> (Item, String) {
    let res = s
        .client
        .post(s.url("/v1/items"))
        .json(&json!({ "name": name, "description": "d" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let etag = etag(&res);
    (res.json().await.unwrap(), etag)
}

fn etag(res: &reqwest::Response) -> String {
    let etag = res.headers()["etag"].to_str().unwrap().to_owned();
    assert!(etag.starts_with('"') && etag.ends_with('"'), "{etag}");
    assert_eq!(res.headers()["cache-control"], "private, no-cache");
    etag
}

async fn error_body(res: reqwest::Response, status: StatusCode) -> Problem {
    assert_eq!(res.status(), status);
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let path = res.url().path().to_owned();
    let request_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body: Problem = res.json().await.unwrap();
    assert!(request_id.is_some());
    assert_eq!(body.request_id, request_id);
    assert_eq!(body.status, status.as_u16());
    assert_eq!(body.title, status.canonical_reason().unwrap());
    assert_eq!(body.instance.as_deref(), Some(path.as_str()));
    assert!(body.r#type.ends_with(&body.code), "{body:?}");
    assert!(!body.detail.is_empty());
    body
}

#[tokio::test]
async fn create_returns_201_and_location() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .post(s.url("/v1/items"))
        .json(&json!({ "name": "widget" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let location = res.headers()["location"].to_str().unwrap().to_owned();
    let item: Item = res.json().await.unwrap();
    assert_eq!(location, format!("/v1/items/{}", item.id));
    assert_eq!(item.name, "widget");
    assert_eq!(item.description, None);
    assert_eq!(item.created_at, item.updated_at);
}

#[tokio::test]
async fn create_rejects_invalid_body() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .post(s.url("/v1/items"))
        .json(&json!({ "name": "" }))
        .send()
        .await
        .unwrap();
    let body = error_body(res, StatusCode::UNPROCESSABLE_ENTITY).await;
    assert_eq!(body.code, "validation");
    let errors = body.errors.unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].field, "name");
    assert_eq!(errors[0].code, "blank");

    let res = s
        .client
        .post(s.url("/v1/items"))
        .json(&json!({ "name": "x".repeat(201) }))
        .send()
        .await
        .unwrap();
    let body = error_body(res, StatusCode::UNPROCESSABLE_ENTITY).await;
    let errors = body.errors.unwrap();
    assert_eq!(errors[0].field, "name");
    assert_eq!(errors[0].code, "length");

    let res = s
        .client
        .post(s.url("/v1/items"))
        .json(&json!({ "description": "no name" }))
        .send()
        .await
        .unwrap();
    let body = error_body(res, StatusCode::UNPROCESSABLE_ENTITY).await;
    assert_eq!(body.code, "validation");
    assert!(body.errors.is_some());

    let res = s
        .client
        .post(s.url("/v1/items"))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    let body = error_body(res, StatusCode::BAD_REQUEST).await;
    assert_eq!(body.code, "bad_request");
    assert!(body.errors.is_none());
}

#[tokio::test]
async fn list_pages() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let created: Vec<Item> = {
        let mut v = Vec::new();
        for i in 0..3 {
            v.push(create(&s, &format!("list-{i}")).await);
        }
        v
    };

    let page: Page = s
        .client
        .get(s.url("/v1/items?limit=2"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.limit, 2);
    assert_eq!(page.offset, 0);
    assert!(page.total >= 3, "{}", page.total);

    // Walk every page; all created items must show up exactly once.
    let mut seen = Vec::new();
    let mut offset = 0;
    loop {
        let page: Page = s
            .client
            .get(s.url(&format!("/v1/items?limit=2&offset={offset}")))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if page.items.is_empty() {
            break;
        }
        offset += page.items.len() as u32;
        seen.extend(page.items.into_iter().map(|i| i.id));
    }
    for item in &created {
        assert_eq!(seen.iter().filter(|id| **id == item.id).count(), 1);
    }

    for q in ["limit=0", "limit=101"] {
        let res = s
            .client
            .get(s.url(&format!("/v1/items?{q}")))
            .send()
            .await
            .unwrap();
        let body = error_body(res, StatusCode::UNPROCESSABLE_ENTITY).await;
        assert_eq!(body.code, "validation");
        assert_eq!(body.errors.unwrap()[0].field, "limit");
    }
    for q in ["limit=x", "offset=-1"] {
        let res = s
            .client
            .get(s.url(&format!("/v1/items?{q}")))
            .send()
            .await
            .unwrap();
        let body = error_body(res, StatusCode::BAD_REQUEST).await;
        assert_eq!(body.code, "bad_request");
    }
}

#[tokio::test]
async fn get_found_and_missing() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let item = create(&s, "get").await;
    let res = s
        .client
        .get(s.url(&format!("/v1/items/{}", item.id)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let got: Item = res.json().await.unwrap();
    assert_eq!(got, item);

    let res = s
        .client
        .get(s.url(&format!("/v1/items/{}", Uuid::new_v4())))
        .send()
        .await
        .unwrap();
    let body = error_body(res, StatusCode::NOT_FOUND).await;
    assert_eq!(body.code, "not_found");
}

#[tokio::test]
async fn invalid_uuid_is_400() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url("/v1/items/nope")).send().await.unwrap();
    let body = error_body(res, StatusCode::BAD_REQUEST).await;
    assert_eq!(body.code, "bad_request");
}

#[tokio::test]
async fn update_replaces() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let item = create(&s, "before").await;
    let url = s.url(&format!("/v1/items/{}", item.id));

    let res = s
        .client
        .put(&url)
        .json(&json!({ "name": "after" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated: Item = res.json().await.unwrap();
    assert_eq!(updated.id, item.id);
    assert_eq!(updated.name, "after");
    assert_eq!(updated.description, None);
    assert_eq!(updated.created_at, item.created_at);
    assert!(updated.updated_at >= item.updated_at);

    let got: Item = s
        .client
        .get(&url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got, updated);

    let res = s
        .client
        .put(&url)
        .json(&json!({ "name": "" }))
        .send()
        .await
        .unwrap();
    error_body(res, StatusCode::UNPROCESSABLE_ENTITY).await;

    let res = s
        .client
        .put(s.url(&format!("/v1/items/{}", Uuid::new_v4())))
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .unwrap();
    error_body(res, StatusCode::NOT_FOUND).await;
}

#[tokio::test]
async fn delete_then_404() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let item = create(&s, "gone").await;
    let url = s.url(&format!("/v1/items/{}", item.id));

    let res = s.client.delete(&url).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(res.bytes().await.unwrap().is_empty());

    let res = s.client.delete(&url).send().await.unwrap();
    error_body(res, StatusCode::NOT_FOUND).await;

    let res = s.client.get(&url).send().await.unwrap();
    error_body(res, StatusCode::NOT_FOUND).await;
}

#[tokio::test]
async fn conditional_get() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let (item, created_etag) = create_with_etag(&s, "etag").await;
    let url = s.url(&format!("/v1/items/{}", item.id));

    let res = s.client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tag = etag(&res);
    assert_eq!(tag, created_etag);

    for value in [tag.as_str(), "*", "\"other\", W/\"x\""] {
        let res = s
            .client
            .get(&url)
            .header("if-none-match", value)
            .send()
            .await
            .unwrap();
        if value.contains("other") {
            assert_eq!(res.status(), StatusCode::OK, "{value}");
        } else {
            assert_eq!(res.status(), StatusCode::NOT_MODIFIED, "{value}");
            assert_eq!(etag(&res), tag);
            assert!(res.bytes().await.unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn if_match_on_writes() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let (item, tag) = create_with_etag(&s, "if-match").await;
    let url = s.url(&format!("/v1/items/{}", item.id));

    let res = s
        .client
        .put(&url)
        .header("if-match", "\"stale\"")
        .json(&json!({ "name": "lost" }))
        .send()
        .await
        .unwrap();
    let body = error_body(res, StatusCode::PRECONDITION_FAILED).await;
    assert_eq!(body.code, "precondition_failed");

    let res = s
        .client
        .put(&url)
        .header("if-match", &tag)
        .json(&json!({ "name": "won" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let new_tag = etag(&res);
    assert_ne!(new_tag, tag);
    let updated: Item = res.json().await.unwrap();
    assert_eq!(updated.name, "won");

    let res = s
        .client
        .delete(&url)
        .header("if-match", &tag)
        .send()
        .await
        .unwrap();
    error_body(res, StatusCode::PRECONDITION_FAILED).await;

    let res = s
        .client
        .delete(&url)
        .header("if-match", &new_tag)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = s
        .client
        .delete(&url)
        .header("if-match", &new_tag)
        .send()
        .await
        .unwrap();
    error_body(res, StatusCode::NOT_FOUND).await;
}

#[tokio::test]
async fn error_body_shape() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.client.get(s.url("/v1/nothing")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let v: Value = res.json().await.unwrap();
    let obj = v.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "code",
            "detail",
            "instance",
            "request_id",
            "status",
            "title",
            "type"
        ],
        "{v}"
    );
    assert_eq!(obj["code"], "not_found");
    assert_eq!(obj["status"], 404);
    assert_eq!(obj["title"], "Not Found");
    assert_eq!(obj["instance"], "/v1/nothing");
    assert!(obj["type"].as_str().unwrap().ends_with(":error:not_found"));
    assert!(obj["detail"].is_string());
    assert!(obj["request_id"].is_string());
}
