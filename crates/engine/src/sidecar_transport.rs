// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use ringo_am_api::{ApiTransport, TransportError};
use serde_json::Value;

use crate::protocol::ErrorCode;
use crate::{Error, SidecarEngine};

/// [`ApiTransport`] that sends the requests through the engine with `api.get`.
#[derive(Clone, Debug)]
pub struct SidecarTransport {
    engine: Arc<SidecarEngine>,
}

impl SidecarTransport {
    /// Creates a transport that uses `engine`.
    #[must_use]
    pub fn new(engine: Arc<SidecarEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl ApiTransport for SidecarTransport {
    async fn get(&self, path: &str, query: &[(&str, &str)]) -> ringo_am_api::Result<Value> {
        let query: BTreeMap<String, String> = query
            .iter()
            .map(|&(key, value)| (key.to_owned(), value.to_owned()))
            .collect();
        let response = self.engine.api_get(path, query).await;
        response.map_err(|error| ringo_am_api::Error::Transport(map_error(&error)))
    }
}

/// Maps an engine error to a transport error.
fn map_error(error: &Error) -> TransportError {
    match error {
        Error::Remote {
            code: ErrorCode::NotAuthorized,
            ..
        } => TransportError::NotAuthorized,
        Error::Remote {
            code: ErrorCode::NoSubscription,
            ..
        } => TransportError::NoSubscription,
        Error::Remote {
            code: ErrorCode::NotReady,
            ..
        }
        | Error::NotReady => TransportError::NotReady,
        other => TransportError::Other {
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(code: ErrorCode) -> Error {
        Error::Remote {
            code,
            message: "failed".to_owned(),
        }
    }

    #[test]
    fn map_error_maps_the_protocol_codes() {
        assert_eq!(
            map_error(&remote(ErrorCode::NotAuthorized)),
            TransportError::NotAuthorized
        );
        assert_eq!(
            map_error(&remote(ErrorCode::NoSubscription)),
            TransportError::NoSubscription
        );
        assert_eq!(
            map_error(&remote(ErrorCode::NotReady)),
            TransportError::NotReady
        );
        assert_eq!(map_error(&Error::NotReady), TransportError::NotReady);
    }

    #[test]
    fn map_error_keeps_the_message_of_other_errors() {
        let error = map_error(&remote(ErrorCode::MusicKitError));
        let TransportError::Other { message } = error else {
            panic!("expected Other, got {error:?}");
        };
        assert!(message.contains("failed"), "message was: {message}");
    }
}
