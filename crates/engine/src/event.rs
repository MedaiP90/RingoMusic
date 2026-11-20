// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use crate::ItemId;
use crate::protocol::{ErrorCode, FromEngine, PlaybackState};

/// Event that a [`PlaybackEngine`](crate::PlaybackEngine) sends to its subscribers.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum EngineEvent {
    /// The engine is ready for requests.
    Ready,
    /// A self-check of the engine failed. Playback is not possible.
    Incompatible {
        /// Name of the failed check.
        check: String,
        /// Details of the failure.
        detail: String,
    },
    /// The authorization changed.
    AuthChanged {
        /// The user authorized the app.
        authorized: bool,
    },
    /// The playback state changed.
    State {
        /// New playback state.
        state: PlaybackState,
    },
    /// The current item changed.
    NowPlaying {
        /// Item ID.
        id: ItemId,
        /// Item title.
        title: String,
        /// Artist name.
        artist: String,
        /// Album name.
        album: String,
        /// Duration of the item.
        duration: Duration,
        /// Artwork URL, if the item has artwork.
        artwork_url: Option<String>,
        /// ID of the album, if the engine knows it.
        album_id: Option<String>,
        /// Index of the item in the queue that the caller sent.
        queue_index: usize,
    },
    /// The playback position changed.
    Position {
        /// Position in the current item.
        position: Duration,
    },
    /// The engine reports an error.
    Error {
        /// Error code.
        code: ErrorCode,
        /// Error message.
        message: String,
        /// If `true`, the engine quits after this error.
        fatal: bool,
    },
    /// The supervisor started a new engine process after a crash.
    ///
    /// The new engine has no queue. The caller must restore it.
    Restarted,
    /// The supervisor stopped to restart the engine because it failed too often.
    GaveUp,
}

impl EngineEvent {
    /// Converts a notification. Returns `None` for notifications that are not events.
    #[must_use]
    pub fn from_notification(notification: FromEngine) -> Option<Self> {
        let event = match notification {
            FromEngine::EngineHello { .. } | FromEngine::EngineLog { .. } => return None,
            FromEngine::EngineReady => Self::Ready,
            FromEngine::EngineIncompatible { check, detail } => {
                Self::Incompatible { check, detail }
            }
            FromEngine::AuthChanged { authorized } => Self::AuthChanged { authorized },
            FromEngine::PlayerState { state } => Self::State { state },
            FromEngine::PlayerNowPlaying {
                id,
                title,
                artist,
                album,
                duration_ms,
                artwork_url,
                album_id,
                queue_index,
            } => Self::NowPlaying {
                id: ItemId::new(id),
                title,
                artist,
                album,
                duration: Duration::from_millis(duration_ms),
                artwork_url,
                album_id,
                queue_index: usize::try_from(queue_index).unwrap_or(usize::MAX),
            },
            FromEngine::PlayerPosition { position_ms } => Self::Position {
                position: Duration::from_millis(position_ms),
            },
            FromEngine::Error {
                code,
                message,
                fatal,
            } => Self::Error {
                code,
                message,
                fatal,
            },
        };
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_and_log_are_not_events() {
        let hello = FromEngine::EngineHello {
            proto: 1,
            engine_version: "0.1.0".to_owned(),
            electron: "41.0.0".to_owned(),
        };
        assert_eq!(EngineEvent::from_notification(hello), None);
    }

    #[test]
    fn now_playing_converts_units() {
        let notification = FromEngine::PlayerNowPlaying {
            id: "42".to_owned(),
            title: "Title".to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            duration_ms: 1500,
            artwork_url: None,
            album_id: None,
            queue_index: 3,
        };
        let Some(EngineEvent::NowPlaying {
            id,
            duration,
            queue_index,
            ..
        }) = EngineEvent::from_notification(notification)
        else {
            panic!("expected a NowPlaying event");
        };
        assert_eq!(id.as_str(), "42");
        assert_eq!(duration, Duration::from_millis(1500));
        assert_eq!(queue_index, 3);
    }
}
