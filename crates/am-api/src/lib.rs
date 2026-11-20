// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Typed client for the Apple Music API.

mod artwork;
mod error;
mod fixture_transport;
mod transport;

pub use artwork::artwork_url;
pub use error::{Error, Result, TransportError};
pub use fixture_transport::FixtureTransport;
pub use transport::ApiTransport;
