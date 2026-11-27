// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Typed client for the Apple Music API.

pub mod endpoints;
pub mod models;
#[cfg(feature = "write")]
pub mod write;

mod artwork;
mod error;
mod page;
mod transport;

pub use artwork::artwork_url;
pub use error::{Error, Result, TransportError};
pub use page::{PAGE_SIZE, Page};
pub use transport::{ApiTransport, WriteMethod, WriteRequest};

#[cfg(any(test, feature = "fixture"))]
mod fixture_transport;

#[cfg(any(test, feature = "fixture"))]
pub use fixture_transport::{FixtureTransport, RecordedWrite};
