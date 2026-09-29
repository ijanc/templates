// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! The GraphQL schema. Every root type is a `MergedObject`, so a new
//! resource adds its own `Query`/`Mutation` structs next to [`items`]
//! and one field here.

use async_graphql::{EmptySubscription, MergedObject};

use crate::store::Store;

pub mod error;
pub mod items;

/// Deepest selection set accepted.
pub const DEPTH_LIMIT: usize = 10;

/// Highest complexity accepted; every field counts 1.
pub const COMPLEXITY_LIMIT: usize = 200;

#[derive(Default, MergedObject)]
pub struct Query(items::ItemQuery);

#[derive(Default, MergedObject)]
pub struct Mutation(items::ItemMutation);

pub type Schema = async_graphql::Schema<Query, Mutation, EmptySubscription>;

/// The schema, resolving against `store`.
pub fn schema(store: Store) -> Schema {
    Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .data(store)
        .limit_depth(DEPTH_LIMIT)
        .limit_complexity(COMPLEXITY_LIMIT)
        .finish()
}
