// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

/// Error of the API client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The transport could not get a response.
    #[error("API request failed")]
    Transport(#[from] TransportError),
}

/// Result type of the API client.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Reason why a transport could not get a response.
#[derive(Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The user did not authorize the app in Apple Music.
    #[error("not authorized")]
    NotAuthorized,

    /// The account has no Apple Music subscription.
    #[error("no Apple Music subscription")]
    NoSubscription,

    /// The transport is not ready yet, for example during an engine restart.
    #[error("transport is not ready")]
    NotReady,

    /// The transport has no response for the request.
    #[error("not found: {message}")]
    NotFound {
        /// Description of the request.
        message: String,
    },

    /// Any other failure.
    #[error("{message}")]
    Other {
        /// Description of the failure.
        message: String,
    },
}
