// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! MPRIS server. It shows the playback state on the D-Bus session bus, so the
//! desktop shell, media keys, and tools like `playerctl` can control the player.
//!
//! The crate reads snapshots of [`AppState`](ringo_core::state::AppState) and
//! sends [`Message`](ringo_core::state::Message) values to the core. Use
//! [`run`] to start it.

mod callbacks;
mod capabilities;
mod error;
mod mapping;
mod player;
mod position;
mod service;
mod snapshot;

pub use callbacks::Callbacks;
pub use error::{Error, Result};
pub use service::run;
