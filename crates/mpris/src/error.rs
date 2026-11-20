// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::error::Error as StdError;

/// Error of the MPRIS server.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The server cannot use the session bus or cannot get the bus name. For
    /// example, another instance of the application owns the name.
    #[error("cannot publish the player on the session bus")]
    Bus(#[source] Box<dyn StdError + Send + Sync>),
}

/// Result type of this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn bus(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Bus(Box::new(source))
    }
}
