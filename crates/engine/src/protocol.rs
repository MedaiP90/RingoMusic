// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Wire protocol between Rust and the Electron engine.
//!
//! The Rust types in this module are the source of truth. `protocol/schema.json`
//! and `engine/src/protocol.ts` are generated from them. `protocol/PROTOCOL.md`
//! describes the behavior.

mod envelope;
mod error_code;
mod from_engine;
mod schema;
mod to_engine;

pub use envelope::{
    ErrorObject, ErrorResponse, JsonRpcVersion, Notification, Request, SuccessResponse,
};
pub use error_code::{ErrorCode, UnknownErrorCode};
pub use from_engine::{FromEngine, LogLevel, PlaybackState};
pub use schema::ProtocolSchema;
pub use to_engine::{AuthStatus, QueueKind, RepeatMode, ToEngine};

/// Protocol version that the engine sends in `engine.hello`.
pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests;
