// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Item queries and mutations over HTTP.

mod common;

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::{FieldError, Item, Page, Problem, TestServer, data, error};

const CREATE_Q: &str = "mutation($input: CreateItemInput!) { \
     createItem(input: $input) { id name description createdAt updatedAt } }";

const GET_Q: &str = "query($id: UUID!) { \
     item(id: $id) { id name description createdAt updatedAt } }";

const UPDATE_Q: &str = "mutation($id: UUID!, $input: UpdateItemInput!) { \
     updateItem(id: $id, input: $input) { \
     id name description createdAt updatedAt } }";

const DELETE_Q: &str = "mutation($id: UUID!) { deleteItem(id: $id) }";

const LIST_Q: &str = "query($limit: Int, $offset: Int) { \
     items(limit: $limit, offset: $offset) { \
     items { id name description createdAt updatedAt } total limit offset } }";

async fn create(s: &TestServer, name: &str) -> Item {
    let input = json!({ "name": name, "description": "d" });
    let res = s.gql(CREATE_Q, json!({ "input": input })).await;
    data(&res, "createItem")
}

fn field_errors(e: &Value) -> Vec<FieldError> {
    serde_json::from_value(e["extensions"]["errors"].clone()).unwrap()
}

#[tokio::test]
async fn create_returns_item() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .gql(CREATE_Q, json!({ "input": { "name": "widget" } }))
        .await;
    let item: Item = data(&res, "createItem");
    assert_eq!(item.name, "widget");
    assert_eq!(item.description, None);
    assert_eq!(item.created_at, item.updated_at);
}

#[tokio::test]
async fn create_rejects_invalid_input() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.gql(CREATE_Q, json!({ "input": { "name": "" } })).await;
    assert!(res["data"].is_null(), "{res}");
    let e = error(&res, "BAD_USER_INPUT");
    assert_eq!(e["path"], json!(["createItem"]));
    let errors = field_errors(e);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].field, "name");
    assert_eq!(errors[0].code, "blank");

    let input = json!({ "name": "x".repeat(201) });
    let res = s.gql(CREATE_Q, json!({ "input": input })).await;
    let errors = field_errors(error(&res, "BAD_USER_INPUT"));
    assert_eq!(errors[0].field, "name");
    assert_eq!(errors[0].code, "length");

    // Missing required field: rejected by the schema before any resolver.
    let input = json!({ "description": "no name" });
    let res = s.gql(CREATE_Q, json!({ "input": input })).await;
    error(&res, "BAD_REQUEST");

    let res = s.gql("{ nope }", json!({})).await;
    error(&res, "BAD_REQUEST");
}

#[tokio::test]
async fn malformed_body_is_400() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .post(s.url(common::GRAPHQL_PATH))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(res.headers()["content-type"], "application/problem+json");
    let body: Problem = res.json().await.unwrap();
    assert_eq!(body.code, "bad_request");
    assert_eq!(body.instance.as_deref(), Some(common::GRAPHQL_PATH));
    assert!(body.request_id.is_some());
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

    let page: Page = data(&s.gql(LIST_Q, json!({ "limit": 2 })).await, "items");
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.limit, 2);
    assert_eq!(page.offset, 0);
    assert!(page.total >= 3, "{}", page.total);

    // Walk every page; all created items must show up exactly once.
    let mut seen = Vec::new();
    let mut offset = 0;
    loop {
        let vars = json!({ "limit": 2, "offset": offset });
        let page: Page = data(&s.gql(LIST_Q, vars).await, "items");
        if page.items.is_empty() {
            break;
        }
        offset += page.items.len() as u32;
        seen.extend(page.items.into_iter().map(|i| i.id));
    }
    for item in &created {
        assert_eq!(seen.iter().filter(|id| **id == item.id).count(), 1);
    }

    for limit in [0, 101] {
        let res = s.gql(LIST_Q, json!({ "limit": limit })).await;
        let errors = field_errors(error(&res, "BAD_USER_INPUT"));
        assert_eq!(errors[0].field, "limit");
    }
    for vars in [json!({ "limit": "x" }), json!({ "offset": -1 })] {
        let res = s.gql(LIST_Q, vars).await;
        error(&res, "BAD_REQUEST");
    }
}

#[tokio::test]
async fn get_found_and_missing() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let item = create(&s, "get").await;
    let res = s.gql(GET_Q, json!({ "id": item.id })).await;
    let got: Item = data(&res, "item");
    assert_eq!(got, item);

    let res = s.gql(GET_Q, json!({ "id": Uuid::new_v4() })).await;
    let got: Option<Item> = data(&res, "item");
    assert_eq!(got, None);
}

#[tokio::test]
async fn invalid_uuid() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.gql(GET_Q, json!({ "id": "nope" })).await;
    error(&res, "BAD_REQUEST");
}

#[tokio::test]
async fn update_replaces() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let item = create(&s, "before").await;

    let vars = json!({ "id": item.id, "input": { "name": "after" } });
    let updated: Item = data(&s.gql(UPDATE_Q, vars).await, "updateItem");
    assert_eq!(updated.id, item.id);
    assert_eq!(updated.name, "after");
    assert_eq!(updated.description, None);
    assert_eq!(updated.created_at, item.created_at);
    assert!(updated.updated_at >= item.updated_at);

    let got: Item = data(&s.gql(GET_Q, json!({ "id": item.id })).await, "item");
    assert_eq!(got, updated);

    let vars = json!({ "id": item.id, "input": { "name": "" } });
    error(&s.gql(UPDATE_Q, vars).await, "BAD_USER_INPUT");

    let vars = json!({ "id": Uuid::new_v4(), "input": { "name": "x" } });
    error(&s.gql(UPDATE_Q, vars).await, "NOT_FOUND");
}

#[tokio::test]
async fn delete_then_not_found() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let item = create(&s, "gone").await;
    let vars = json!({ "id": item.id });

    let id: Uuid = data(&s.gql(DELETE_Q, vars.clone()).await, "deleteItem");
    assert_eq!(id, item.id);

    error(&s.gql(DELETE_Q, vars.clone()).await, "NOT_FOUND");

    let got: Option<Item> = data(&s.gql(GET_Q, vars).await, "item");
    assert_eq!(got, None);
}

#[tokio::test]
async fn request_id_in_errors() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s
        .client
        .post(s.url(common::GRAPHQL_PATH))
        .header("x-request-id", "abc-123")
        .json(&json!({ "query": "{ nope }" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.headers()["x-request-id"], "abc-123");
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["errors"][0]["extensions"]["request_id"], "abc-123");
}

#[tokio::test]
async fn depth_limited() {
    let Some(s) = common::spawn().await else {
        return;
    };
    // Twelve levels, deeper than the limit.
    let q = ["{ __schema { types {", &" ofType {".repeat(9), " name"].concat()
        + &" }".repeat(12);
    let res = s.gql(&q, json!({})).await;
    assert!(res.get("data").is_none_or(Value::is_null), "{res}");
    error(&res, "BAD_REQUEST");
}
