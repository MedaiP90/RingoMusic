// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Finds the one edit that turns the shown list into the new list. The queue
//! view uses it to change its rows without a rebuild.

/// A single edit of a list.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Edit {
    /// The item at the position is gone.
    Remove(usize),
    /// A new item is at the position.
    Insert(usize),
    /// The item at `from` is at `to` now.
    Move { from: usize, to: usize },
}

/// Returns the edit that turns `shown` into `want`. Returns `None` if the
/// lists are the same, or if more than one edit is needed.
pub fn single_edit<T: PartialEq>(shown: &[T], want: &[T]) -> Option<Edit> {
    if shown == want {
        return None;
    }
    let prefix = shown.iter().zip(want).take_while(|(a, b)| a == b).count();
    let shortest = shown.len().min(want.len());
    let suffix = shown
        .iter()
        .rev()
        .zip(want.iter().rev())
        .take(shortest - prefix)
        .take_while(|(a, b)| a == b)
        .count();
    let kept = prefix + suffix;
    if shown.len() == want.len() + 1 && kept == want.len() {
        return Some(Edit::Remove(prefix));
    }
    if want.len() == shown.len() + 1 && kept == shown.len() {
        return Some(Edit::Insert(prefix));
    }
    if shown.len() != want.len() || kept + 2 > shown.len() {
        return None;
    }
    let last = shown.len() - suffix - 1;
    if shown[prefix] == want[last] && shown[prefix + 1..=last] == want[prefix..last] {
        return Some(Edit::Move {
            from: prefix,
            to: last,
        });
    }
    if shown[last] == want[prefix] && shown[prefix..last] == want[prefix + 1..=last] {
        return Some(Edit::Move {
            from: last,
            to: prefix,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_lists_need_no_edit() {
        assert_eq!(single_edit(&["a", "b"], &["a", "b"]), None);
    }

    #[test]
    fn finds_a_removal() {
        assert_eq!(
            single_edit(&["a", "b", "c"], &["b", "c"]),
            Some(Edit::Remove(0))
        );
        assert_eq!(
            single_edit(&["a", "b", "c"], &["a", "c"]),
            Some(Edit::Remove(1))
        );
        assert_eq!(
            single_edit(&["a", "b", "c"], &["a", "b"]),
            Some(Edit::Remove(2))
        );
    }

    #[test]
    fn finds_an_insertion() {
        assert_eq!(
            single_edit(&["a", "c"], &["a", "b", "c"]),
            Some(Edit::Insert(1))
        );
        assert_eq!(single_edit::<&str>(&[], &["a"]), Some(Edit::Insert(0)));
    }

    #[test]
    fn finds_a_move_in_both_directions() {
        assert_eq!(
            single_edit(&["a", "b", "c", "d"], &["a", "c", "d", "b"]),
            Some(Edit::Move { from: 1, to: 3 })
        );
        assert_eq!(
            single_edit(&["a", "b", "c", "d"], &["a", "d", "b", "c"]),
            Some(Edit::Move { from: 3, to: 1 })
        );
        assert_eq!(
            single_edit(&["a", "b"], &["b", "a"]),
            Some(Edit::Move { from: 0, to: 1 })
        );
    }

    #[test]
    fn gives_up_on_larger_changes() {
        assert_eq!(single_edit(&["a", "b", "c"], &["x", "y", "z"]), None);
        assert_eq!(single_edit(&["a", "b", "c"], &["c"]), None);
        assert_eq!(
            single_edit(&["a", "b", "c", "d"], &["b", "a", "d", "c"]),
            None
        );
    }

    #[test]
    fn handles_repeated_items() {
        assert_eq!(
            single_edit(&["a", "a", "b"], &["a", "b"]),
            Some(Edit::Remove(1))
        );
    }
}
