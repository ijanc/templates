// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! HTTP validators for items: `ETag` on every item response, `304` for
//! `If-None-Match` on reads and `412` for `If-Match` on writes.

use axum::http::{HeaderMap, HeaderValue, header};
use axum_extra::headers::{ETag, HeaderMapExt};

use crate::model::Item;

/// Strong validator derived from `updated_at` in microseconds; the
/// stores keep at most that precision, so the tag of a freshly created
/// item matches the tag of the same item read back.
pub fn etag(item: &Item) -> ETag {
    format!("\"{}\"", item.updated_at.timestamp_micros())
        .parse()
        .expect("quoted integer is a valid etag")
}

/// Sent with every item: only the requesting client may keep a copy,
/// and it must be revalidated (`If-None-Match`) before each use.
pub const CACHE_CONTROL: HeaderValue =
    HeaderValue::from_static("private, no-cache");

/// `ETag` and `Cache-Control` for `item`.
pub fn headers(item: &Item) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.typed_insert(etag(item));
    h.insert(header::CACHE_CONTROL, CACHE_CONTROL);
    h
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use super::*;

    fn item(updated_at: DateTime<Utc>) -> Item {
        Item {
            id: Uuid::nil(),
            name: "x".into(),
            description: None,
            created_at: updated_at,
            updated_at,
        }
    }

    #[test]
    fn ignores_sub_microsecond_precision() {
        let t = Utc::now();
        let micros =
            DateTime::from_timestamp_micros(t.timestamp_micros()).unwrap();
        assert_eq!(etag(&item(t)), etag(&item(micros)));
    }

    #[test]
    fn changes_with_updated_at() {
        let t = Utc::now();
        let later = t + chrono::Duration::microseconds(1);
        assert_ne!(etag(&item(t)), etag(&item(later)));
    }

    #[test]
    fn response_headers() {
        let h = headers(&item(Utc::now()));
        assert!(h.typed_get::<ETag>().is_some());
        assert_eq!(h["cache-control"], "private, no-cache");
    }
}
