// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Playback queue, state, and storage shared by the front ends.

pub mod api_cache;
pub mod db;

mod error;

pub use error::{Error, Result};
