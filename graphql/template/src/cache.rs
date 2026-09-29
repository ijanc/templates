// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! In-process read cache in front of the store, one entry per item.
//! Writes go through the store and update or drop the entry, so this
//! process never serves stale reads; other replicas would, until the
//! TTL runs out.

use std::time::Duration;

use axum_prometheus::metrics::counter;
use moka::future::Cache as Moka;
use uuid::Uuid;

use crate::{metrics, model::Item};

/// Limits of the cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Entries are dropped this long after being written.
    pub ttl: Duration,
    /// Most entries kept; least recently used ones go first.
    pub capacity: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(60),
            capacity: 10_000,
        }
    }
}

/// Items by id; cheap to clone, clones share the entries.
#[derive(Clone, Debug)]
pub struct Cache {
    items: Moka<Uuid, Item>,
}

impl Cache {
    pub fn new(cfg: &Config) -> Self {
        let items = Moka::builder()
            .time_to_live(cfg.ttl)
            .max_capacity(cfg.capacity)
            .build();
        Self { items }
    }

    /// Counts a hit or a miss in `<prefix>_cache_hits_total` and
    /// `<prefix>_cache_misses_total`.
    pub async fn get(&self, id: Uuid) -> Option<Item> {
        let item = self.items.get(&id).await;
        let name = if item.is_some() {
            "cache_hits_total"
        } else {
            "cache_misses_total"
        };
        counter!(format!("{}_{name}", metrics::PREFIX)).increment(1);
        item
    }

    pub async fn insert(&self, item: &Item) {
        self.items.insert(item.id, item.clone()).await;
    }

    pub async fn invalidate(&self, id: Uuid) {
        self.items.invalidate(&id).await;
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn item() -> Item {
        let now = Utc::now();
        Item {
            id: Uuid::new_v4(),
            name: "x".into(),
            description: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn insert_get_invalidate() {
        let c = Cache::new(&Config::default());
        let it = item();
        assert_eq!(c.get(it.id).await, None);
        c.insert(&it).await;
        assert_eq!(c.get(it.id).await, Some(it.clone()));
        c.invalidate(it.id).await;
        assert_eq!(c.get(it.id).await, None);
    }

    #[tokio::test]
    async fn expires() {
        let c = Cache::new(&Config {
            ttl: Duration::from_millis(10),
            capacity: 1,
        });
        let it = item();
        c.insert(&it).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(c.get(it.id).await, None);
    }
}
