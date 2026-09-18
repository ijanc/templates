// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Item pages. Reading is public; every write needs an account and
//! goes through a form post that redirects, so a reload never repeats
//! it.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::{Html, Response},
    routing::{MethodRouter, get, post},
};
use minijinja::context;
use uuid::Uuid;
use validator::Validate;

use crate::{
    AppState, auth,
    error::Error,
    extract::{Form, Path, Query},
    flash::Flash,
    model::{CreateItem, Item, ListQuery, UpdateItem, field_errors},
    render::Renderer,
};

/// Mounted under `/items`.
pub fn router(state: &AppState) -> Router<AppState> {
    let guard =
        middleware::from_fn_with_state(state.clone(), auth::require_auth);
    let guarded = |m: MethodRouter<AppState>| m.route_layer(guard.clone());
    Router::new()
        .route("/", get(list).merge(guarded(post(create))))
        .route("/new", guarded(get(new)))
        .route("/{id}", get(show).merge(guarded(post(update))))
        .route("/{id}/edit", guarded(get(edit)))
        .route("/{id}/delete", guarded(post(destroy)))
}

fn not_found(id: Uuid) -> Error {
    Error::NotFound(format!("item {id} not found"))
}

async fn load(state: &AppState, id: Uuid) -> Result<Item, Error> {
    state.store.get(id).await?.ok_or_else(|| not_found(id))
}

/// One page of items, oldest first.
pub async fn list(
    State(state): State<AppState>,
    r: Renderer,
    Query(q): Query<ListQuery>,
) -> Result<Html<String>, Error> {
    let (limit, offset) = q.limits();
    let page = state.store.list(limit, offset).await?;
    let (prev, next) = (page.prev(), page.next());
    r.render("items/index.html", context! { page, prev, next })
}

/// The empty create form.
pub async fn new(r: Renderer) -> Result<Html<String>, Error> {
    let form = CreateItem::default();
    r.render("items/form.html", form_context(&form, "/items", "New item"))
}

/// Store a new item, or send the form back with the messages.
pub async fn create(
    State(state): State<AppState>,
    r: Renderer,
    Form(input): Form<CreateItem>,
) -> Result<Response, Error> {
    if let Err(e) = input.validate() {
        let ctx = context! {
            errors => field_errors(&e),
            ..form_context(&input, "/items", "New item")
        };
        return r.render_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "items/form.html",
            ctx,
        );
    }
    let item = state.store.create(input).await?;
    let to = format!("/items/{}", item.id);
    r.redirect(&to, Flash::success("item created")).await
}

/// One item.
pub async fn show(
    State(state): State<AppState>,
    r: Renderer,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Error> {
    let item = load(&state, id).await?;
    r.render("items/show.html", context! { item })
}

/// The edit form, filled in.
pub async fn edit(
    State(state): State<AppState>,
    r: Renderer,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Error> {
    let item = load(&state, id).await?;
    let form = UpdateItem::from(&item);
    let action = format!("/items/{id}");
    let ctx = context! {
        item,
        ..form_context(&form, &action, "Edit item")
    };
    r.render("items/form.html", ctx)
}

/// Replace an item, or send the form back with the messages.
pub async fn update(
    State(state): State<AppState>,
    r: Renderer,
    Path(id): Path<Uuid>,
    Form(input): Form<UpdateItem>,
) -> Result<Response, Error> {
    let action = format!("/items/{id}");
    if let Err(e) = input.validate() {
        let ctx = context! {
            errors => field_errors(&e),
            ..form_context(&input, &action, "Edit item")
        };
        return r.render_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "items/form.html",
            ctx,
        );
    }
    state
        .store
        .update(id, input)
        .await?
        .ok_or_else(|| not_found(id))?;
    r.redirect(&action, Flash::success("item saved")).await
}

/// Delete an item and go back to the list.
pub async fn destroy(
    State(state): State<AppState>,
    r: Renderer,
    Path(id): Path<Uuid>,
) -> Result<Response, Error> {
    if !state.store.delete(id).await? {
        return Err(not_found(id));
    }
    r.redirect("/items", Flash::success("item deleted")).await
}

/// What `items/form.html` needs, whatever it is editing.
fn form_context(
    form: &impl serde::Serialize,
    action: &str,
    title: &str,
) -> minijinja::Value {
    context! {
        form,
        action,
        title,
        errors => minijinja::Value::from(std::collections::BTreeMap::<
            String,
            Vec<String>,
        >::new()),
    }
}
