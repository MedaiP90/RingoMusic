// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Playback queue model. It has no I/O and does not call the engine.
//!
//! The queue keeps the order of the user and an optional shuffle order. The
//! engine gets the playback order (see [`Queue::playback_order`]) and plays it
//! in sequence. After each change, [`QueueChange`] says what the engine needs.

mod queue_change;
mod queue_item;
mod queue_model;

pub use queue_change::QueueChange;
pub use queue_item::QueueItem;
pub use queue_model::Queue;

#[cfg(test)]
mod tests;
