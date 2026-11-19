// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Supervisor and client for the Electron playback engine.

use std::io;

use protocol::ErrorCode;

pub mod protocol;
pub mod rpc;

pub use rpc::RpcConnection;

/// Error of the engine client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A read or write on the socket failed.
    #[error("socket I/O failed")]
    Io(#[from] io::Error),

    /// The engine did not send `engine.hello` in time.
    #[error("engine did not send engine.hello in time")]
    HandshakeTimeout,

    /// The engine speaks another protocol version.
    #[error("protocol version mismatch: expected {expected}, engine uses {actual}")]
    ProtocolMismatch {
        /// Version that this client supports.
        expected: u32,
        /// Version that the engine reported.
        actual: u32,
    },

    /// A message is not valid, or it is not allowed at this point.
    #[error("invalid message: {detail}")]
    InvalidMessage {
        /// What is wrong with the message.
        detail: String,
    },

    /// The engine answered a request with an error.
    #[error("engine returned error {code:?}: {message}")]
    Remote {
        /// Error code of the engine.
        code: ErrorCode,
        /// Error message of the engine.
        message: String,
    },

    /// The engine did not answer a request in time.
    #[error("engine did not answer in time")]
    RequestTimeout,

    /// The socket closed.
    #[error("connection to the engine is closed")]
    ConnectionClosed,
}

/// Result type of the engine client.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidMessage {
            detail: error.to_string(),
        }
    }
}
