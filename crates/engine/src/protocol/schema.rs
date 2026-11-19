// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use schemars::JsonSchema;

use super::envelope::{ErrorResponse, Notification, Request, SuccessResponse};
use super::from_engine::FromEngine;
use super::to_engine::{AuthStatus, ToEngine};

/// Root type that only exists to put all protocol definitions in one schema.
/// It is never sent on the wire.
#[derive(Clone, Debug, PartialEq, JsonSchema, serde::Deserialize, serde::Serialize)]
pub struct ProtocolSchema {
    /// Request envelope.
    pub request: Request,
    /// Notification envelope.
    pub notification: Notification,
    /// Success answer envelope.
    pub success_response: SuccessResponse,
    /// Error answer envelope.
    pub error_response: ErrorResponse,
    /// All methods from Rust to the engine.
    pub to_engine: ToEngine,
    /// All notifications from the engine to Rust.
    pub from_engine: FromEngine,
    /// Result of `auth.status`.
    pub auth_status: AuthStatus,
}
