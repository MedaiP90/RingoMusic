// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use ringo_am_api::{ApiTransport, TransportError, WriteMethod, WriteRequest};
use serde_json::Value;

use crate::protocol::{self, ApiWriteRequest, ErrorCode};
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

    async fn get_uncached(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> ringo_am_api::Result<Value> {
        // This transport has no cache, so it is the same as `get`.
        self.get(path, query).await
    }

    async fn write(&self, request: WriteRequest<'_>) -> ringo_am_api::Result<Option<Value>> {
        let response = self.engine.api_write(to_engine_request(request)).await;
        response
            .map(non_null)
            .map_err(|error| ringo_am_api::Error::Transport(map_error(&error)))
    }
}

/// Converts a write request to the params of `api.write`.
fn to_engine_request(request: WriteRequest<'_>) -> ApiWriteRequest {
    let method = match request.method {
        WriteMethod::Post => protocol::WriteMethod::Post,
        WriteMethod::Put => protocol::WriteMethod::Put,
        WriteMethod::Delete => protocol::WriteMethod::Delete,
    };
    ApiWriteRequest {
        method,
        path: request.path.to_owned(),
        query: request
            .query
            .iter()
            .map(|&(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        body: request.body.cloned(),
    }
}

/// Maps the `null` of an empty response to `None`.
fn non_null(value: Value) -> Option<Value> {
    (!value.is_null()).then_some(value)
}

/// Maps an engine error to a transport error.
fn map_error(error: &Error) -> TransportError {
    if let Some(rejected) = rejection(error) {
        return rejected;
    }
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
        Error::Remote {
            code: ErrorCode::NotFound,
            message,
            ..
        } => TransportError::NotFound {
            message: message.clone(),
        },
        other => TransportError::Other {
            message: other.to_string(),
        },
    }
}

/// Reads the HTTP status and the Apple error code that the engine adds to a failed write.
fn rejection(error: &Error) -> Option<TransportError> {
    let Error::Remote {
        data: Some(data), ..
    } = error
    else {
        return None;
    };
    let status = u16::try_from(data.get("status")?.as_u64()?).ok()?;
    let apple_code = data
        .get("apple_code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(TransportError::Rejected { status, apple_code })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(code: ErrorCode) -> Error {
        Error::Remote {
            code,
            message: "failed".to_owned(),
            data: None,
        }
    }

    #[test]
    fn to_engine_request_copies_every_field() {
        let body = serde_json::json!({"a": 1});
        let request = to_engine_request(WriteRequest {
            method: WriteMethod::Delete,
            path: "/v1/me/ratings/songs/1",
            query: &[("l", "en")],
            body: Some(&body),
        });

        assert_eq!(request.method, protocol::WriteMethod::Delete);
        assert_eq!(request.path, "/v1/me/ratings/songs/1");
        assert_eq!(request.query.get("l").map(String::as_str), Some("en"));
        assert_eq!(request.body, Some(body));
    }

    #[test]
    fn non_null_maps_an_empty_response_to_none() {
        assert_eq!(non_null(Value::Null), None);
        assert_eq!(non_null(serde_json::json!({})), Some(serde_json::json!({})));
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
        assert_eq!(
            map_error(&remote(ErrorCode::NotFound)),
            TransportError::NotFound {
                message: "failed".to_owned()
            }
        );
    }

    #[test]
    fn map_error_keeps_the_status_and_the_apple_code_of_a_rejected_write() {
        let error = Error::Remote {
            code: ErrorCode::NoSubscription,
            message: "failed".to_owned(),
            data: Some(serde_json::json!({"status": 403, "apple_code": "40300"})),
        };
        assert_eq!(
            map_error(&error),
            TransportError::Rejected {
                status: 403,
                apple_code: Some("40300".to_owned())
            }
        );
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
