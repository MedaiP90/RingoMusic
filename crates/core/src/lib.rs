// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Playback queue, state, and storage shared by the front ends.

pub mod api_cache;
pub mod db;
pub mod paths;
pub mod queue;
pub mod runner;
pub mod state;

mod error;
#[cfg(any(test, feature = "mock"))]
mod mock_engine;

pub use error::{Error, Result};

#[cfg(any(test, feature = "mock"))]
pub use mock_engine::{EngineCall, MockEngine};
