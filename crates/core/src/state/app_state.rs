// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use ringo_engine::ItemId;
use ringo_engine::protocol::PlaybackState;

use crate::queue::Queue;

/// State of the playback engine process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineStatus {
    /// The engine starts and is not ready yet.
    Starting,
    /// The engine accepts requests.
    Ready,
    /// A self-check of the engine failed. Playback is not possible.
    Incompatible {
        /// Name of the failed check.
        check: String,
        /// Details of the failure.
        detail: String,
    },
    /// The engine speaks another protocol version. The supervisor does not start
    /// it again.
    ProtocolMismatch {
        /// Version that the app supports.
        expected: u32,
        /// Version that the engine reported.
        actual: u32,
    },
    /// The engine crashed and the supervisor starts it again.
    Restarting,
    /// The engine failed too often. The supervisor does not start it again.
    GaveUp,
    /// The Chromium sandbox of the engine cannot start. The supervisor does not
    /// start the engine again.
    SandboxFailed,
}

/// Authorization of the user.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthState {
    /// The engine did not report the authorization yet.
    Unknown,
    /// The user did not authorize the app.
    SignedOut,
    /// The user authorized the app.
    SignedIn {
        /// Storefront ID, for example `us`.
        storefront: String,
    },
}

/// Playback state, position, and volume.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Playback {
    /// State that the engine reported last.
    pub state: PlaybackState,
    /// Position in the current item.
    pub position: Duration,
    /// Volume from 0.0 to 1.0.
    pub volume: f32,
}

/// The item that the engine plays, as the engine reported it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NowPlaying {
    /// Item ID.
    pub id: ItemId,
    /// Item title.
    pub title: String,
    /// Artist name.
    pub artist: String,
    /// Album name.
    pub album: String,
    /// Duration of the item.
    pub duration: Duration,
    /// Artwork URL, if the item has artwork.
    pub artwork_url: Option<String>,
    /// Album ID, if the engine knows it.
    pub album_id: Option<String>,
}

/// Playback to resume when the engine is ready: after a start or a restart.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Resume {
    /// Position in the current item.
    pub(crate) position: Duration,
    /// `true` if playback must start again.
    pub(crate) play: bool,
}

/// All state of the application. The fields can be read from everywhere.
/// Only [`update`](super::update) changes them.
#[derive(Clone, Debug)]
pub struct AppState {
    /// State of the engine process.
    pub engine: EngineStatus,
    /// Authorization of the user.
    pub auth: AuthState,
    /// The login window is open.
    pub signing_in: bool,
    /// The last login failed. The user did not cancel it.
    pub sign_in_failed: bool,
    /// The API refused a request because the account has no subscription.
    pub no_subscription: bool,
    /// Playback queue.
    pub queue: Queue,
    /// Playback state, position, and volume.
    pub playback: Playback,
    /// The item that plays now.
    pub now_playing: Option<NowPlaying>,
    /// Set until the engine is ready and has the queue again.
    pub(crate) resume: Option<Resume>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Creates the state at application start: engine starting, nobody known,
    /// empty queue, full volume.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: EngineStatus::Starting,
            auth: AuthState::Unknown,
            signing_in: false,
            sign_in_failed: false,
            no_subscription: false,
            queue: Queue::new(),
            playback: Playback {
                state: PlaybackState::Stopped,
                position: Duration::ZERO,
                volume: 1.0,
            },
            now_playing: None,
            resume: None,
        }
    }
}
