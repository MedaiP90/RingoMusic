// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use ringo_engine::ItemId;

/// One change of the upcoming items of the engine queue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Edit {
    /// Removes the item at an index of the engine queue.
    Remove(usize),
    /// Inserts items after the current item.
    PlayNext(Vec<ItemId>),
    /// Adds items at the end.
    PlayLater(Vec<ItemId>),
}

/// The queue as the engine has it. MusicKit cannot replace the upcoming items
/// at once, so the runner compares this copy with the wanted order and sends
/// only the difference.
#[derive(Debug, Default)]
pub(super) struct EngineQueue {
    items: Vec<ItemId>,
    current: usize,
    /// Rust index minus engine index for the items from the current item on.
    /// It is not zero when the engine keeps items before the current item
    /// that the Rust queue does not have in the same place.
    offset: isize,
}

impl EngineQueue {
    /// The engine got a new queue.
    pub(super) fn set(&mut self, items: Vec<ItemId>, current: usize) {
        self.items = items;
        self.current = current;
        self.offset = 0;
    }

    /// The engine queue now has the same items as the Rust queue from the
    /// current item on. `rust_current` is the index of the current item in
    /// the Rust queue.
    pub(super) fn align(&mut self, rust_current: usize) {
        self.offset = to_isize(rust_current) - to_isize(self.current);
    }

    /// Converts an index of the engine queue to an index of the Rust queue.
    pub(super) fn to_rust_index(&self, engine_index: usize) -> usize {
        usize::try_from(to_isize(engine_index) + self.offset).unwrap_or(usize::MAX)
    }

    /// Returns `true` when engine and Rust indexes are the same.
    pub(super) fn is_aligned(&self) -> bool {
        self.offset == 0
    }

    /// The engine plays another item.
    pub(super) fn set_current(&mut self, current: usize) {
        self.current = current;
    }

    /// The engine moved to the next item.
    pub(super) fn step_forward(&mut self) {
        self.current = (self.current + 1).min(self.items.len().saturating_sub(1));
    }

    /// Records an edit that the engine did.
    pub(super) fn apply(&mut self, edit: &Edit) {
        match edit {
            Edit::Remove(index) => {
                self.items.remove(*index);
            }
            Edit::PlayNext(items) => {
                let at = (self.current + 1).min(self.items.len());
                self.items.splice(at..at, items.iter().cloned());
            }
            Edit::PlayLater(items) => self.items.extend(items.iter().cloned()),
        }
    }

    /// Returns the edits that change the items after the current item to
    /// the items of `target`, or `None` if the current items are not the same.
    ///
    /// The engine can insert items only after the current item. So the items
    /// before the current item stay as they are. [`EngineQueue::align`] keeps
    /// the indexes right.
    pub(super) fn plan(&self, target: &[ItemId], target_current: usize) -> Option<Vec<Edit>> {
        let engine_current = self.items.get(self.current)?;
        if target.get(target_current) != Some(engine_current) {
            return None;
        }
        let engine_tail = &self.items[self.current + 1..];
        let target_tail = &target[target_current + 1..];
        Some(plan_tail(engine_tail, target_tail, self.current + 1))
    }
}

fn to_isize(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

/// Changes the items after the current item. `start` is the engine index of
/// the first of them.
fn plan_tail(engine: &[ItemId], target: &[ItemId], start: usize) -> Vec<Edit> {
    let common = engine
        .iter()
        .zip(target)
        .take_while(|(a, b)| a == b)
        .count();
    if common == engine.len() {
        return append(&target[common..]);
    }
    if target.ends_with(engine) {
        let inserted = &target[..target.len() - engine.len()];
        return vec![Edit::PlayNext(inserted.to_vec())];
    }
    if let Some(indexes) = removals(engine, target) {
        return indexes
            .into_iter()
            .map(|index| Edit::Remove(start + index))
            .collect();
    }
    let mut edits: Vec<Edit> = (common..engine.len())
        .rev()
        .map(|index| Edit::Remove(start + index))
        .collect();
    edits.extend(append(&target[common..]));
    edits
}

/// Returns the indexes of the `engine` items to remove so that `target`
/// stays, highest index first. Returns `None` if `target` is not a
/// subsequence of `engine`.
fn removals(engine: &[ItemId], target: &[ItemId]) -> Option<Vec<usize>> {
    let mut indexes = Vec::new();
    let mut wanted = target.iter().rev().peekable();
    // The indexes go down, so each removal keeps the next index valid.
    for (index, item) in engine.iter().enumerate().rev() {
        if wanted.peek() == Some(&item) {
            wanted.next();
        } else {
            indexes.push(index);
        }
    }
    wanted.next().is_none().then_some(indexes)
}

fn append(items: &[ItemId]) -> Vec<Edit> {
    if items.is_empty() {
        Vec::new()
    } else {
        vec![Edit::PlayLater(items.to_vec())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &str) -> Vec<ItemId> {
        names
            .chars()
            .map(|name| ItemId::new(name.to_string()))
            .collect()
    }

    fn queue(names: &str, current: usize) -> EngineQueue {
        let mut queue = EngineQueue::default();
        queue.set(ids(names), current);
        queue
    }

    /// Plans, applies the edits to the copy, and returns the items after the
    /// current item.
    fn planned_tail(engine: &str, current: usize, target: &str, target_current: usize) -> String {
        let mut queue = queue(engine, current);
        let edits = queue
            .plan(&ids(target), target_current)
            .expect("the change is possible");
        for edit in &edits {
            queue.apply(edit);
        }
        queue.items[queue.current + 1..]
            .iter()
            .map(ItemId::as_str)
            .collect()
    }

    #[test]
    fn plan_returns_no_edit_for_equal_queues() {
        assert_eq!(queue("abc", 1).plan(&ids("abc"), 1), Some(Vec::new()));
    }

    #[test]
    fn plan_appends_new_items_with_play_later() {
        assert_eq!(
            queue("abc", 0).plan(&ids("abcde"), 0),
            Some(vec![Edit::PlayLater(ids("de"))])
        );
    }

    #[test]
    fn plan_inserts_items_after_current_with_play_next() {
        assert_eq!(
            queue("abc", 0).plan(&ids("axybc"), 0),
            Some(vec![Edit::PlayNext(ids("xy"))])
        );
    }

    #[test]
    fn plan_removes_upcoming_item_with_descending_indexes() {
        assert_eq!(
            queue("abcd", 0).plan(&ids("ac"), 0),
            Some(vec![Edit::Remove(3), Edit::Remove(1)])
        );
    }

    #[test]
    fn plan_keeps_items_before_current() {
        assert_eq!(queue("abc", 2).plan(&ids("c"), 0), Some(Vec::new()));
        assert_eq!(queue("bc", 1).plan(&ids("abc"), 2), Some(Vec::new()));
    }

    #[test]
    fn plan_returns_none_when_current_item_differs() {
        assert_eq!(queue("abc", 0).plan(&ids("bac"), 0), None);
    }

    #[test]
    fn plan_returns_none_for_index_outside_queue() {
        assert_eq!(queue("abc", 5).plan(&ids("abc"), 0), None);
        assert_eq!(queue("abc", 0).plan(&ids("abc"), 5), None);
    }

    #[test]
    fn apply_of_plan_reaches_the_target_tail() {
        let cases = [
            ("abcde", 2, "acxde", 1),
            ("abcde", 0, "aedcb", 0),
            ("abcde", 4, "exyz", 0),
            ("abc", 1, "bxc", 0),
            ("a", 0, "axyz", 0),
            ("abc", 1, "ab", 1),
            ("abab", 1, "ab", 1),
        ];
        for (engine, current, target, target_current) in cases {
            assert_eq!(
                planned_tail(engine, current, target, target_current),
                target[target_current + 1..],
                "engine {engine}@{current} to target {target}@{target_current}"
            );
        }
    }

    #[test]
    fn align_maps_engine_indexes_to_rust_indexes() {
        let mut queue = queue("abcd", 1);
        queue.align(0);
        assert!(!queue.is_aligned());
        assert_eq!(queue.to_rust_index(2), 1);
    }
}
