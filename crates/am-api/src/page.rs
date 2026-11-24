// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use crate::models::{ApiPage, Relationship};

/// Number of items that a list request asks for.
pub const PAGE_SIZE: u32 = 25;

/// One page of a list, with the offset of the next page.
#[derive(Clone, Debug, PartialEq)]
pub struct Page<T> {
    /// Items of this page.
    pub items: Vec<T>,
    /// Offset to request the next page. `None` on the last page.
    pub next_offset: Option<u32>,
}

/// An empty last page. A derive would add a `T: Default` bound.
impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            next_offset: None,
        }
    }
}

impl<T> From<ApiPage<T>> for Page<T> {
    fn from(page: ApiPage<T>) -> Self {
        let next_offset = page.next.as_deref().and_then(parse_next_offset);
        Self {
            items: page.data.into_vec(),
            next_offset,
        }
    }
}

impl<T> From<Relationship<T>> for Page<T> {
    fn from(relationship: Relationship<T>) -> Self {
        let next_offset = relationship.next.as_deref().and_then(parse_next_offset);
        Self {
            items: relationship.data.into_vec(),
            next_offset,
        }
    }
}

/// Reads the `offset` parameter of a `next` URL. Returns `None` if it is
/// missing or not a number.
fn parse_next_offset(next: &str) -> Option<u32> {
    let (_, query) = next.split_once('?')?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("offset="))
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relationship_converts_to_a_page_with_the_next_offset() {
        let relationship: Relationship<u32> = serde_json::from_str(
            r#"{ "data": [1, 2], "next": "/v1/me/library/playlists/p.x/tracks?offset=2" }"#,
        )
        .expect("the test relationship is valid");

        let page = Page::from(relationship);

        assert_eq!((page.items, page.next_offset), (vec![1, 2], Some(2)));
    }

    #[test]
    fn parse_next_offset_reads_the_offset_parameter() {
        let next = "/v1/catalog/it/search?offset=5&term=radiohead&types=songs";

        assert_eq!(parse_next_offset(next), Some(5));
    }

    #[test]
    fn parse_next_offset_reads_the_last_parameter() {
        assert_eq!(
            parse_next_offset("/v1/me/library/songs?limit=25&offset=50"),
            Some(50)
        );
    }

    #[test]
    fn parse_next_offset_returns_none_without_offset() {
        assert_eq!(parse_next_offset("/v1/me/library/songs?limit=25"), None);
        assert_eq!(parse_next_offset("/v1/me/library/songs"), None);
    }

    #[test]
    fn parse_next_offset_returns_none_for_a_bad_number() {
        assert_eq!(parse_next_offset("/v1/me/library/songs?offset=abc"), None);
    }
}
