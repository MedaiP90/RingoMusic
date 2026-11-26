// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use serde::Deserialize;
use serde_json::Value;

use crate::protocol::{ErrorResponse, FromEngine, Notification, SuccessResponse};
use crate::{Error, Result};

/// One message that the engine sends.
#[derive(Debug)]
pub(super) enum Incoming {
    /// A notification, without `id`.
    Notification(FromEngine),
    /// An answer to a request.
    Response {
        /// ID of the request. `None` if the engine could not read the request.
        id: Option<u64>,
        /// Result or error of the method.
        outcome: Result<Value>,
    },
}

impl Incoming {
    /// Parses one NDJSON line.
    pub(super) fn parse(line: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(line)?;
        if value.get("method").is_some() {
            let notification = Notification::deserialize(value)?;
            Ok(Self::Notification(notification.event))
        } else if value.get("error").is_some() {
            let response = ErrorResponse::deserialize(value)?;
            Ok(Self::Response {
                id: response.id,
                outcome: Err(Error::Remote {
                    code: response.error.code,
                    message: response.error.message,
                    data: response.error.data,
                }),
            })
        } else {
            let response = SuccessResponse::deserialize(value)?;
            Ok(Self::Response {
                id: Some(response.id),
                outcome: Ok(response.result),
            })
        }
    }
}
