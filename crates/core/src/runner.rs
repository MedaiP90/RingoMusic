// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Connects [`AppState`](crate::state::AppState) to a
//! [`PlaybackEngine`](ringo_engine::PlaybackEngine).
//!
//! [`Core`] owns the state. It reads messages from an inbox, applies
//! [`update`](crate::state::update), runs the commands, and publishes a
//! snapshot of the state after each message.

mod core;
mod engine_queue;
mod engine_worker;
mod handle;

pub use self::core::Core;
pub use handle::CoreHandle;
