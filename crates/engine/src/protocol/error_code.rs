// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fmt;

/// Error code in a JSON-RPC error or in an `error` notification.
///
/// Standard JSON-RPC codes and application codes in the range -32000 to -32099.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(try_from = "i32", into = "i32")]
#[schemars(with = "i32")]
#[non_exhaustive]
pub enum ErrorCode {
    /// The engine received invalid JSON (-32700).
    ParseError,
    /// The message is not a valid JSON-RPC object (-32600).
    InvalidRequest,
    /// The method does not exist (-32601).
    MethodNotFound,
    /// The params are not valid for the method (-32602).
    InvalidParams,
    /// The engine had an internal fault (-32603).
    InternalError,
    /// The user did not authorize the app in Apple Music (-32001).
    NotAuthorized,
    /// The account has no Apple Music subscription (-32002).
    NoSubscription,
    /// Widevine or MusicKit is not ready yet (-32003).
    NotReady,
    /// The user cancelled the operation, for example the login (-32004).
    Cancelled,
    /// MusicKit returned an error (-32005).
    MusicKitError,
    /// The Chromium sandbox of the engine cannot start (-32006).
    SandboxFailed,
}

impl ErrorCode {
    /// Returns the numeric code on the wire.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
            Self::NotAuthorized => -32001,
            Self::NoSubscription => -32002,
            Self::NotReady => -32003,
            Self::Cancelled => -32004,
            Self::MusicKitError => -32005,
            Self::SandboxFailed => -32006,
        }
    }
}

/// The numeric value is not a known [`ErrorCode`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnknownErrorCode(pub i32);

impl fmt::Display for UnknownErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown error code {}", self.0)
    }
}

impl std::error::Error for UnknownErrorCode {}

impl From<ErrorCode> for i32 {
    fn from(code: ErrorCode) -> Self {
        code.code()
    }
}

impl TryFrom<i32> for ErrorCode {
    type Error = UnknownErrorCode;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            -32700 => Ok(Self::ParseError),
            -32600 => Ok(Self::InvalidRequest),
            -32601 => Ok(Self::MethodNotFound),
            -32602 => Ok(Self::InvalidParams),
            -32603 => Ok(Self::InternalError),
            -32001 => Ok(Self::NotAuthorized),
            -32002 => Ok(Self::NoSubscription),
            -32003 => Ok(Self::NotReady),
            -32004 => Ok(Self::Cancelled),
            -32005 => Ok(Self::MusicKitError),
            -32006 => Ok(Self::SandboxFailed),
            other => Err(UnknownErrorCode(other)),
        }
    }
}
