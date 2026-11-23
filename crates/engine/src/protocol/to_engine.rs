// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::BTreeMap;

/// Method that Rust sends to the engine, with typed params.
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
#[serde(tag = "method", content = "params")]
pub enum ToEngine {
    /// Returns [`AuthStatus`].
    #[serde(rename = "auth.status")]
    AuthStatus,
    /// Shows the login window. Returns when the login completes or the user cancels.
    #[serde(rename = "auth.signIn")]
    AuthSignIn,
    /// Signs the user out.
    #[serde(rename = "auth.signOut")]
    AuthSignOut,
    /// Replaces the queue. Rust owns the order, including shuffle.
    #[serde(rename = "player.setQueue")]
    PlayerSetQueue {
        /// Catalog or library IDs, in play order.
        items: Vec<String>,
        /// Index in `items` of the first item to play.
        start_index: u32,
        /// Kind of the IDs in `items`.
        kind: QueueKind,
    },
    /// Inserts items after the current item. The current item keeps playing.
    #[serde(rename = "player.playNext")]
    PlayerPlayNext {
        /// Catalog or library IDs, in play order.
        items: Vec<String>,
        /// Kind of the IDs in `items`.
        kind: QueueKind,
    },
    /// Adds items at the end of the queue. The current item keeps playing.
    #[serde(rename = "player.playLater")]
    PlayerPlayLater {
        /// Catalog or library IDs, in play order.
        items: Vec<String>,
        /// Kind of the IDs in `items`.
        kind: QueueKind,
    },
    /// Removes the item at `index` from the queue. The current item cannot be removed.
    #[serde(rename = "player.remove")]
    PlayerRemove {
        /// Index in the queue that the engine plays.
        index: u32,
    },
    /// Starts or resumes playback.
    #[serde(rename = "player.play")]
    PlayerPlay,
    /// Pauses playback.
    #[serde(rename = "player.pause")]
    PlayerPause,
    /// Stops playback.
    #[serde(rename = "player.stop")]
    PlayerStop,
    /// Plays the next queue item.
    #[serde(rename = "player.next")]
    PlayerNext,
    /// Plays the previous queue item.
    #[serde(rename = "player.previous")]
    PlayerPrevious,
    /// Seeks in the current item.
    #[serde(rename = "player.seek")]
    PlayerSeek {
        /// New position in milliseconds.
        position_ms: u64,
    },
    /// Sets the volume.
    #[serde(rename = "player.setVolume")]
    PlayerSetVolume {
        /// Volume from 0.0 to 1.0.
        volume: f32,
    },
    /// Sets the repeat mode.
    #[serde(rename = "player.setRepeat")]
    PlayerSetRepeat {
        /// New repeat mode.
        mode: RepeatMode,
    },
    /// Runs a GET request with `api.music()`. Returns the raw JSON response.
    #[serde(rename = "api.get")]
    ApiGet {
        /// API path, for example `/v1/catalog/us/search`.
        path: String,
        /// Query parameters.
        query: BTreeMap<String, String>,
    },
    /// Asks the engine to quit.
    #[serde(rename = "engine.shutdown")]
    EngineShutdown,
}

/// Kind of the IDs in a queue.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum QueueKind {
    /// Song IDs. Catalog IDs and library IDs (`i.…`) both work.
    Songs,
}

/// Repeat mode of the player.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// No repeat.
    None,
    /// Repeat the current item.
    One,
    /// Repeat the whole queue.
    All,
}

/// Result of `auth.status`.
#[derive(
    Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
pub struct AuthStatus {
    /// The user authorized the app.
    pub authorized: bool,
    /// Storefront ID, for example `us`.
    pub storefront: String,
}
