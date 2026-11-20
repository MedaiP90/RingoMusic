// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Application state and the pure function that changes it.
//!
//! [`AppState`] changes only through [`update`]. It takes a [`Message`] and
//! returns the [`Command`]s that the runner sends to the engine.

mod app_state;
mod command;
mod message;
mod update;

pub use app_state::{AppState, AuthState, EngineStatus, NowPlaying, Playback};
pub use command::Command;
pub use message::Message;
pub use update::update;

#[cfg(test)]
mod tests;
