// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Helpers for lists that load page by page: the state of the loading, and
//! the scroll trigger that asks for the next page.

use std::rc::Rc;

use relm4::gtk;
use relm4::gtk::prelude::*;

/// Tracks which page to load next. It allows one load at a time, and it
/// stops when the last page is loaded.
#[derive(Debug, Default)]
pub struct PageCursor {
    next_offset: Option<u32>,
    loading: bool,
    /// Number of the last reset. The result of an older list is dropped.
    generation: u64,
}

impl PageCursor {
    /// Starts a new list at the first page. A load that runs is now stale.
    pub fn reset(&mut self) {
        self.generation += 1;
        self.next_offset = Some(0);
        self.loading = false;
    }

    /// Takes the next page. Returns the generation and the offset to request,
    /// or `None` if a load runs or no page is left.
    pub fn begin(&mut self) -> Option<(u64, u32)> {
        if self.loading {
            return None;
        }
        let offset = self.next_offset?;
        self.loading = true;
        Some((self.generation, offset))
    }

    /// Records a loaded page. Returns `false` if the load is stale.
    pub fn finish(&mut self, generation: u64, next_offset: Option<u32>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.loading = false;
        self.next_offset = next_offset;
        true
    }

    /// Records a failed load. The same page can load again. Returns `false`
    /// if the load is stale.
    pub fn fail(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.loading = false;
        true
    }
}

/// Tells if the view is within one page height of the end of the content.
pub fn is_near_end(value: f64, upper: f64, page_size: f64) -> bool {
    upper - (value + page_size) <= page_size
}

/// Calls `on_near_end` when the user scrolls near the end of the content, and
/// when the content is too short to fill the view.
pub fn connect_near_end(scrolled: &gtk::ScrolledWindow, on_near_end: impl Fn() + 'static) {
    let adjustment = scrolled.vadjustment();
    let on_near_end = Rc::new(on_near_end);
    let check = move |adjustment: &gtk::Adjustment| {
        let (value, upper, page) = (
            adjustment.value(),
            adjustment.upper(),
            adjustment.page_size(),
        );
        if is_near_end(value, upper, page) {
            on_near_end();
        }
    };
    adjustment.connect_value_changed(check.clone());
    adjustment.connect_changed(check);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started() -> PageCursor {
        let mut cursor = PageCursor::default();
        cursor.reset();
        cursor
    }

    #[test]
    fn begin_returns_none_before_the_first_reset() {
        assert_eq!(PageCursor::default().begin(), None);
    }

    #[test]
    fn begin_returns_the_first_offset_after_a_reset() {
        assert_eq!(started().begin(), Some((1, 0)));
    }

    #[test]
    fn begin_allows_one_load_at_a_time() {
        let mut cursor = started();
        cursor.begin();
        assert_eq!(cursor.begin(), None);
    }

    #[test]
    fn finish_allows_the_next_page() {
        let mut cursor = started();
        cursor.begin();
        assert!(cursor.finish(1, Some(25)));
        assert_eq!(cursor.begin(), Some((1, 25)));
    }

    #[test]
    fn finish_without_next_offset_stops_the_loading() {
        let mut cursor = started();
        cursor.begin();
        cursor.finish(1, None);
        assert_eq!(cursor.begin(), None);
    }

    #[test]
    fn fail_allows_the_same_page_again() {
        let mut cursor = started();
        cursor.begin();
        assert!(cursor.fail(1));
        assert_eq!(cursor.begin(), Some((1, 0)));
    }

    #[test]
    fn finish_and_fail_drop_the_result_of_an_older_list() {
        let mut cursor = started();
        cursor.begin();
        cursor.reset();
        assert!(!cursor.finish(1, Some(25)));
        assert!(!cursor.fail(1));
        assert_eq!(cursor.begin(), Some((2, 0)));
    }

    #[test]
    fn is_near_end_is_true_within_one_page_of_the_end() {
        assert!(is_near_end(2000.0, 3000.0, 600.0));
    }

    #[test]
    fn is_near_end_is_false_far_from_the_end() {
        assert!(!is_near_end(0.0, 3000.0, 600.0));
    }

    #[test]
    fn is_near_end_is_true_when_the_content_is_short() {
        assert!(is_near_end(0.0, 400.0, 600.0));
    }
}
