// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

/// What the engine must do after a queue change.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[must_use]
pub enum QueueChange {
    /// The playback order did not change.
    None,
    /// The playback order changed, but the current item is the same.
    /// The engine can get the new order without an interruption.
    UpcomingOnly,
    /// The current item changed or the queue is new. The engine must get the
    /// whole queue and start at the current item.
    Replaced,
}
