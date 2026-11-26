// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

/// Error of the API client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The transport could not get a response.
    #[error("API request failed")]
    Transport(#[from] TransportError),

    /// A request argument is not valid. No request was sent.
    #[error("invalid request: {message}")]
    InvalidInput {
        /// Description of the problem.
        message: String,
    },

    /// The response does not have the expected shape.
    #[error("cannot parse the API response")]
    Parse(#[source] serde_json::Error),

    /// The response has no resource, for example for an unknown ID.
    #[error("the API response has no resource")]
    EmptyResponse,
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

    /// The server rejected a write. The status and the Apple error code are for the log.
    #[error("request rejected with HTTP status {status}")]
    Rejected {
        /// HTTP status of the response.
        status: u16,
        /// Error code in the body of the response, if Apple sent one.
        apple_code: Option<String>,
    },

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
