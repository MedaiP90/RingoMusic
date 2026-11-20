// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::VecDeque;

use rand::Rng;
use rand::seq::SliceRandom;
use ringo_engine::ItemId;
use ringo_engine::protocol::RepeatMode;

use super::{QueueChange, QueueItem};

/// Number of played items that the history keeps.
const HISTORY_LIMIT: usize = 100;

/// Playback queue: items in user order, an optional shuffle order, the repeat
/// mode, and a history of played items.
///
/// An *index* is a place in the user order. A *position* is a place in the
/// playback order. Both are the same when shuffle is off. The current item is
/// always valid when the queue has items.
#[derive(Clone, Debug)]
pub struct Queue {
    items: Vec<QueueItem>,
    /// Index of the current item. `None` only when the queue is empty.
    current: Option<usize>,
    /// Permutation of all indexes. `Some` when shuffle is on.
    shuffle: Option<Vec<usize>>,
    repeat: RepeatMode,
    history: VecDeque<ItemId>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    /// Creates an empty queue with shuffle off and no repeat.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: None,
            shuffle: None,
            repeat: RepeatMode::None,
            history: VecDeque::new(),
        }
    }

    /// Returns the items in user order.
    #[must_use]
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    /// Returns the current item.
    #[must_use]
    pub fn current(&self) -> Option<&QueueItem> {
        self.current.map(|index| &self.items[index])
    }

    /// Returns the index of the current item in the user order.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// Returns the repeat mode.
    #[must_use]
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Returns `true` when shuffle is on.
    #[must_use]
    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle.is_some()
    }

    /// Returns the IDs of the played items, the oldest first. The history
    /// keeps the last 100 items.
    pub fn history(&self) -> impl Iterator<Item = &ItemId> {
        self.history.iter()
    }

    /// Returns the IDs in the order that the engine plays them.
    #[must_use]
    pub fn playback_order(&self) -> Vec<ItemId> {
        (0..self.items.len())
            .map(|position| self.items[self.index_at(position)].id.clone())
            .collect()
    }

    /// Returns the position of the current item in the playback order. This
    /// is the `queue_index` that the engine reports.
    #[must_use]
    pub fn current_playback_index(&self) -> Option<usize> {
        self.current.and_then(|index| self.position_of(index))
    }

    /// Replaces all items. Playback starts at the index `start`, which is
    /// limited to the last item. Shuffle turns off: call [`Self::set_shuffle`]
    /// to shuffle the new items.
    pub fn replace(&mut self, items: Vec<QueueItem>, start: usize) -> QueueChange {
        self.current = items.len().checked_sub(1).map(|last| start.min(last));
        self.items = items;
        self.shuffle = None;
        QueueChange::Replaced
    }

    /// Adds items at the end of the queue. In shuffle mode, they also go to
    /// the end of the playback order.
    pub fn append(&mut self, items: Vec<QueueItem>) -> QueueChange {
        if items.is_empty() {
            return QueueChange::None;
        }
        let first_new = self.items.len();
        self.items.extend(items);
        let end = self.items.len();
        if let Some(order) = &mut self.shuffle {
            order.extend(first_new..end);
        }
        if self.current.is_some() {
            return QueueChange::UpcomingOnly;
        }
        self.current = Some(self.index_at(0));
        QueueChange::Replaced
    }

    /// Adds items after the current item. They play next in both orders.
    pub fn play_next(&mut self, items: Vec<QueueItem>) -> QueueChange {
        let (Some(current), Some(position)) = (self.current, self.current_playback_index()) else {
            return self.append(items);
        };
        if items.is_empty() {
            return QueueChange::None;
        }
        let count = items.len();
        let at = current + 1;
        self.items.splice(at..at, items);
        if let Some(order) = &mut self.shuffle {
            for index in order.iter_mut().filter(|index| **index >= at) {
                *index += count;
            }
            let after_current = position + 1;
            order.splice(after_current..after_current, at..at + count);
        }
        QueueChange::UpcomingOnly
    }

    /// Removes the item at `index`. If it is the current item, the next item
    /// becomes current, or the previous item when the removed one was last.
    pub fn remove(&mut self, index: usize) -> QueueChange {
        let Some(position) = self.current_playback_index() else {
            return QueueChange::None;
        };
        if index >= self.items.len() {
            return QueueChange::None;
        }
        let removed_current = self.current == Some(index);
        self.items.remove(index);
        if let Some(order) = &mut self.shuffle {
            order.retain(|entry| *entry != index);
            for entry in order.iter_mut().filter(|entry| **entry > index) {
                *entry -= 1;
            }
        }
        let Some(last) = self.items.len().checked_sub(1) else {
            self.current = None;
            return QueueChange::Replaced;
        };
        if removed_current {
            self.current = Some(self.index_at(position.min(last)));
            return QueueChange::Replaced;
        }
        self.current = self.current.map(|current| {
            if current > index {
                current - 1
            } else {
                current
            }
        });
        QueueChange::UpcomingOnly
    }

    /// Moves the item at index `from` to index `to`. The current item stays
    /// the same item.
    pub fn move_item(&mut self, from: usize, to: usize) -> QueueChange {
        let len = self.items.len();
        if from == to || from >= len || to >= len {
            return QueueChange::None;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.current = self.current.map(|index| index_after_move(index, from, to));
        if let Some(order) = &mut self.shuffle {
            for entry in order.iter_mut() {
                *entry = index_after_move(*entry, from, to);
            }
        }
        QueueChange::UpcomingOnly
    }

    /// Removes all items. The shuffle setting stays, the history stays.
    pub fn clear(&mut self) -> QueueChange {
        if self.items.is_empty() {
            return QueueChange::None;
        }
        self.items.clear();
        self.current = None;
        self.shuffle = self.shuffle.as_ref().map(|_| Vec::new());
        QueueChange::Replaced
    }

    /// Makes the item at `index` current. Returns the new position in the
    /// playback order, or `None` if `index` is not valid.
    pub fn jump_to(&mut self, index: usize) -> Option<usize> {
        let position = self.position_of(index)?;
        self.move_to(position);
        Some(position)
    }

    /// Moves to the next position when a song ends. Repeat one stays on the
    /// same item. Repeat all goes from the last item to the first.
    /// Returns the new position, or `None` when the queue ends.
    pub fn advance(&mut self) -> Option<usize> {
        let position = self.current_playback_index()?;
        let next = match self.repeat {
            RepeatMode::One => position,
            RepeatMode::All => (position + 1) % self.items.len(),
            RepeatMode::None => {
                let next = position + 1;
                if next >= self.items.len() {
                    return None;
                }
                next
            }
        };
        self.move_to(next);
        Some(next)
    }

    /// Moves to the previous position. With repeat all, the first item goes
    /// to the last item. Returns the new position, or `None` at the start.
    pub fn go_back(&mut self) -> Option<usize> {
        let position = self.current_playback_index()?;
        let previous = match position.checked_sub(1) {
            Some(previous) => previous,
            None if self.repeat == RepeatMode::All => self.items.len() - 1,
            None => return None,
        };
        self.move_to(previous);
        Some(previous)
    }

    /// Follows the engine: it reported `queue_index` as the playing position.
    /// Returns `false` and changes nothing if the position is not valid.
    pub fn sync_to_playback_index(&mut self, queue_index: usize) -> bool {
        if queue_index >= self.items.len() {
            return false;
        }
        if self.current_playback_index() != Some(queue_index) {
            self.move_to(queue_index);
        }
        true
    }

    /// Turns shuffle on or off. When it turns on, the current item stays
    /// first and the rest is shuffled with `rng`. When it turns off, the
    /// queue returns to user order at the current item.
    pub fn set_shuffle(&mut self, enabled: bool, rng: &mut impl Rng) -> QueueChange {
        if enabled == self.shuffle.is_some() {
            return QueueChange::None;
        }
        self.shuffle = enabled.then(|| self.shuffled_order(rng));
        if self.items.is_empty() {
            QueueChange::None
        } else {
            QueueChange::UpcomingOnly
        }
    }

    /// Sets the repeat mode. The caller sends it to the engine.
    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Builds a random order of all indexes with the current item first.
    fn shuffled_order(&self, rng: &mut impl Rng) -> Vec<usize> {
        let mut rest: Vec<usize> = (0..self.items.len())
            .filter(|index| Some(*index) != self.current)
            .collect();
        rest.shuffle(rng);
        self.current.into_iter().chain(rest).collect()
    }

    /// Returns the index of the item at `position` in the playback order.
    fn index_at(&self, position: usize) -> usize {
        match &self.shuffle {
            Some(order) => order[position],
            None => position,
        }
    }

    /// Returns the position of the item at `index`, if the index is valid.
    fn position_of(&self, index: usize) -> Option<usize> {
        if index >= self.items.len() {
            return None;
        }
        match &self.shuffle {
            Some(order) => order.iter().position(|entry| *entry == index),
            None => Some(index),
        }
    }

    /// Makes the item at `position` current and records the old one.
    fn move_to(&mut self, position: usize) {
        if let Some(previous) = self.current().map(|item| item.id.clone()) {
            if self.history.len() == HISTORY_LIMIT {
                self.history.pop_front();
            }
            self.history.push_back(previous);
        }
        self.current = Some(self.index_at(position));
    }
}

/// Returns where the item at `index` is after the item at `from` moved to `to`.
fn index_after_move(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if from > to && index >= to && index < from {
        index + 1
    } else {
        index
    }
}
