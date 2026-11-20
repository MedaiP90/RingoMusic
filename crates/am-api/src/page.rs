// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use crate::models::ApiPage;

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

impl<T> From<ApiPage<T>> for Page<T> {
    fn from(page: ApiPage<T>) -> Self {
        let next_offset = page.next.as_deref().and_then(parse_next_offset);
        Self {
            items: page.data.into_vec(),
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
