// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use ringo_engine::ItemId;
use ringo_engine::protocol::RepeatMode;

/// Call that the runner makes on the engine.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Replaces the queue in the engine and starts at the index `start`. The
    /// engine does not play until [`Command::Play`].
    SetQueue {
        /// IDs in play order.
        items: Vec<ItemId>,
        /// Index of the first item.
        start: usize,
    },
    /// Changes the upcoming items in the engine without a stop of the current
    /// item.
    SyncUpcoming {
        /// IDs in play order.
        items: Vec<ItemId>,
        /// Index of the current item in `items`.
        current: usize,
    },
    /// Starts or resumes playback.
    Play,
    /// Pauses playback.
    Pause,
    /// Stops playback.
    Stop,
    /// Plays the next item.
    Next,
    /// Plays an item at once. The engine inserts it after the current item
    /// and skips to it, so the order of the engine queue does not matter.
    /// Use it for the previous item and for a jump in the queue.
    PlayItem {
        /// ID of the item.
        id: ItemId,
    },
    /// Seeks in the current item.
    Seek(Duration),
    /// Sets the volume from 0.0 to 1.0.
    SetVolume(f32),
    /// Sets the repeat mode.
    SetRepeat(RepeatMode),
    /// Reads the authorization from the engine.
    RefreshAuth,
    /// Starts the engine again after it gave up.
    RestartEngine,
    /// Shows the login window.
    SignIn,
    /// Signs the user out.
    SignOut,
}
