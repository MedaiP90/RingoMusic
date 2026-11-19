// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use serde_json::{Map, Value};

use super::error_code::ErrorCode;

/// Notification that the engine sends to Rust.
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
#[serde(tag = "method", content = "params")]
pub enum FromEngine {
    /// First message of each connection.
    #[serde(rename = "engine.hello")]
    EngineHello {
        /// Protocol version of the engine.
        proto: u32,
        /// Version of the engine package.
        engine_version: String,
        /// Version of Electron.
        electron: String,
    },
    /// Widevine is ready and MusicKit is available.
    #[serde(rename = "engine.ready")]
    EngineReady,
    /// A self-check failed.
    #[serde(rename = "engine.incompatible")]
    EngineIncompatible {
        /// Name of the failed check.
        check: String,
        /// Details of the failure.
        detail: String,
    },
    /// A log record. Rust re-emits it with `tracing`, target `engine`.
    #[serde(rename = "engine.log")]
    EngineLog {
        /// Severity of the record.
        level: LogLevel,
        /// Log message.
        message: String,
        /// Structured fields of the record.
        fields: Map<String, Value>,
    },
    /// The authorization changed.
    #[serde(rename = "auth.changed")]
    AuthChanged {
        /// The user authorized the app.
        authorized: bool,
    },
    /// The playback state changed.
    #[serde(rename = "player.state")]
    PlayerState {
        /// New playback state.
        state: PlaybackState,
    },
    /// The current item changed.
    #[serde(rename = "player.nowPlaying")]
    PlayerNowPlaying {
        /// Item ID.
        id: String,
        /// Item title.
        title: String,
        /// Artist name.
        artist: String,
        /// Album name.
        album: String,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Artwork URL, if the item has artwork.
        artwork_url: Option<String>,
        /// Index of the item in the queue that Rust sent.
        queue_index: u32,
    },
    /// The playback position changed. The engine sends at most 4 per second.
    #[serde(rename = "player.position")]
    PlayerPosition {
        /// Position in milliseconds.
        position_ms: u64,
    },
    /// The engine reports an error.
    #[serde(rename = "error")]
    Error {
        /// Error code.
        code: ErrorCode,
        /// Error message.
        message: String,
        /// If `true`, the engine quits after this notification.
        fatal: bool,
    },
}

/// Severity of an `engine.log` record.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    /// Fine-grained trace.
    Trace,
    /// Debug information.
    Debug,
    /// Normal event.
    Info,
    /// Recoverable problem.
    Warn,
    /// Failure.
    Error,
}

/// Playback state of the engine.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    /// An item plays.
    Playing,
    /// Playback is paused.
    Paused,
    /// Playback is stopped.
    Stopped,
    /// An item loads.
    Loading,
    /// The queue ended.
    Ended,
}
