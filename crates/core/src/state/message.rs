// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use ringo_engine::EngineEvent;
use ringo_engine::protocol::{AuthStatus, RepeatMode};

use crate::queue::{Queue, QueueItem};

/// Input of [`update`](super::update): an intent of the user or a fact from
/// the engine.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Message {
    /// Starts or resumes playback.
    Play,
    /// Pauses playback.
    Pause,
    /// Stops playback. The queue stays as it is.
    Stop,
    /// Skips to the next item. With repeat one, the current item restarts.
    Next,
    /// Goes to the previous item. At the start of the queue, the current item
    /// restarts.
    Previous,
    /// Seeks in the current item.
    Seek(Duration),
    /// Sets the volume. Values outside 0.0 to 1.0 are limited.
    SetVolume(f32),
    /// Replaces the queue and starts to play at `start`. Shuffle turns off.
    ReplaceQueue {
        /// New items in user order.
        items: Vec<QueueItem>,
        /// Index of the first item to play.
        start: usize,
    },
    /// Replaces the queue, turns shuffle on, and starts to play at a random
    /// item. The engine gets one new queue.
    ReplaceQueueShuffled {
        /// New items in user order.
        items: Vec<QueueItem>,
        /// Seed of the random number generator.
        seed: u64,
    },
    /// Adds items at the end of the queue.
    Append(Vec<QueueItem>),
    /// Adds items after the current item.
    PlayNext(Vec<QueueItem>),
    /// Removes the item at the index.
    Remove(usize),
    /// Moves an item in the user order.
    Move {
        /// Index of the item.
        from: usize,
        /// New index of the item.
        to: usize,
    },
    /// Turns shuffle on or off. `seed` starts the random order, so the
    /// result is the same for the same message.
    SetShuffle {
        /// New shuffle setting.
        enabled: bool,
        /// Seed of the random number generator.
        seed: u64,
    },
    /// Sets the repeat mode.
    SetRepeat(RepeatMode),
    /// Shows the login window.
    SignIn,
    /// Signs the user out.
    SignOut,
    /// The login window closed.
    SignInEnded {
        /// The login failed. A cancel by the user is not a failure.
        failed: bool,
    },
    /// The result of a subscription check. A refused API request because of a
    /// missing subscription is `active: false`.
    SubscriptionChecked {
        /// The account has an Apple Music subscription.
        active: bool,
    },
    /// Starts the engine again after it gave up.
    RestartEngine,
    /// The engine reported the authorization.
    AuthLoaded(AuthStatus),
    /// Puts a saved queue in place at application start. The engine gets it
    /// when it is ready. Playback does not start.
    RestoreQueue {
        /// The saved queue.
        queue: Queue,
        /// The saved position in the current item.
        position: Duration,
    },
    /// Stops the runner. It saves the queue and stops the engine first.
    Shutdown,
    /// An event of the engine.
    Engine(EngineEvent),
}
