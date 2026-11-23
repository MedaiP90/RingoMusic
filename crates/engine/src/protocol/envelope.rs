// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::error_code::ErrorCode;
use super::from_engine::FromEngine;
use super::to_engine::ToEngine;

/// The constant `"2.0"` in the `jsonrpc` field.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct JsonRpcVersion;

impl Serialize for JsonRpcVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let version = String::deserialize(deserializer)?;
        if version == "2.0" {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "unsupported jsonrpc version {version}"
            )))
        }
    }
}

impl JsonSchema for JsonRpcVersion {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("JsonRpcVersion")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "const": "2.0" })
    }
}

/// Request from Rust to the engine. The engine answers with the same `id`.
#[derive(Clone, Debug, PartialEq, JsonSchema, Deserialize, Serialize)]
pub struct Request {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Request ID, unique in the connection.
    pub id: u64,
    /// Method and params.
    #[serde(flatten)]
    pub call: ToEngine,
}

/// Notification from the engine to Rust. It has no `id` and gets no answer.
#[derive(Clone, Debug, PartialEq, JsonSchema, Deserialize, Serialize)]
pub struct Notification {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Method and params.
    #[serde(flatten)]
    pub event: FromEngine,
}

/// Success answer of the engine. Methods without a result return `null`.
#[derive(Clone, Debug, PartialEq, JsonSchema, Deserialize, Serialize)]
pub struct SuccessResponse {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// ID of the request.
    pub id: u64,
    /// Result of the method. Its shape depends on the method.
    #[schemars(extend("tsType" = "unknown"))]
    pub result: Value,
}

/// Error answer of the engine.
#[derive(Clone, Debug, PartialEq, JsonSchema, Deserialize, Serialize)]
pub struct ErrorResponse {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// ID of the request, or `null` if the engine could not read the request.
    pub id: Option<u64>,
    /// Error details.
    pub error: ErrorObject,
}

/// Error details in an [`ErrorResponse`].
#[derive(Clone, Debug, PartialEq, JsonSchema, Deserialize, Serialize)]
pub struct ErrorObject {
    /// Error code.
    pub code: ErrorCode,
    /// Error message.
    pub message: String,
    /// Optional extra data.
    #[schemars(extend("tsType" = "unknown"))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
