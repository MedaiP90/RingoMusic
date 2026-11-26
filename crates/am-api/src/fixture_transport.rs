// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::{ApiTransport, Error, Result, TransportError, WriteMethod, WriteRequest};

/// Request key: the path and the sorted query parameters.
type RequestKey = (String, Vec<(String, String)>);

/// Write request that a test sent to [`FixtureTransport`].
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedWrite {
    /// HTTP method of the request.
    pub method: WriteMethod,
    /// API path of the request.
    pub path: String,
    /// Query parameters of the request.
    pub query: Vec<(String, String)>,
    /// JSON body of the request.
    pub body: Option<Value>,
}

/// [`ApiTransport`] that answers from recorded JSON files. It is for tests.
///
/// Each entry maps one request to one file in the fixture directory. A request
/// without an entry fails with [`TransportError::NotFound`].
///
/// A write request is answered by the `response.body` of the file for its method
/// and path. The transport records each write request.
#[derive(Debug)]
pub struct FixtureTransport {
    directory: PathBuf,
    entries: HashMap<RequestKey, String>,
    write_entries: HashMap<(WriteMethod, String), String>,
    writes: Mutex<Vec<RecordedWrite>>,
}

impl FixtureTransport {
    /// Creates a transport that reads `tests/fixtures/api/` in the repository root.
    #[must_use]
    pub fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/api");
        Self::in_directory(root)
    }

    /// Creates a transport that reads the files in `directory`.
    #[must_use]
    pub fn in_directory(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            entries: HashMap::new(),
            write_entries: HashMap::new(),
            writes: Mutex::new(Vec::new()),
        }
    }

    /// Answers the request (`path` and `query`) with the content of `file_name`.
    /// The order of the query parameters does not matter.
    #[must_use]
    pub fn with(mut self, path: &str, query: &[(&str, &str)], file_name: &str) -> Self {
        self.entries
            .insert(request_key(path, query), file_name.to_owned());
        self
    }

    /// Answers the write request (`method` and `path`) with the `response.body`
    /// of `file_name`. Use a path like `write/library_add.json`.
    #[must_use]
    pub fn with_write(mut self, method: WriteMethod, path: &str, file_name: &str) -> Self {
        self.write_entries
            .insert((method, path.to_owned()), file_name.to_owned());
        self
    }

    /// Returns the write requests that the transport received, in order.
    ///
    /// # Panics
    ///
    /// Panics if another thread panicked while it held the record.
    #[must_use]
    pub fn recorded_writes(&self) -> Vec<RecordedWrite> {
        self.writes
            .lock()
            .expect("record lock is not poisoned")
            .clone()
    }

    /// Reads and parses a fixture file.
    fn read_json(&self, file_name: &str) -> Result<Value> {
        let file = self.directory.join(file_name);
        // Blocking read is fine here: fixtures are small and only tests use them.
        let content = fs::read_to_string(&file).map_err(|error| {
            failure(TransportError::Other {
                message: format!("cannot read fixture {}: {error}", file.display()),
            })
        })?;
        serde_json::from_str(&content).map_err(|error| {
            failure(TransportError::Other {
                message: format!("fixture {} is not valid JSON: {error}", file.display()),
            })
        })
    }
}

impl Default for FixtureTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiTransport for FixtureTransport {
    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let Some(file_name) = self.entries.get(&request_key(path, query)) else {
            return Err(failure(TransportError::NotFound {
                message: format!("no fixture for {path} with query {query:?}"),
            }));
        };
        self.read_json(file_name)
    }

    async fn get_uncached(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        self.get(path, query).await
    }

    async fn write(&self, request: WriteRequest<'_>) -> Result<Option<Value>> {
        self.writes
            .lock()
            .expect("record lock is not poisoned")
            .push(RecordedWrite {
                method: request.method,
                path: request.path.to_owned(),
                query: request
                    .query
                    .iter()
                    .map(|&(key, value)| (key.to_owned(), value.to_owned()))
                    .collect(),
                body: request.body.cloned(),
            });
        let key = (request.method, request.path.to_owned());
        let Some(file_name) = self.write_entries.get(&key) else {
            return Err(failure(TransportError::NotFound {
                message: format!("no fixture for {:?} {}", request.method, request.path),
            }));
        };
        let fixture = self.read_json(file_name)?;
        // An empty response is stored as `null`.
        Ok(fixture
            .pointer("/response/body")
            .filter(|body| !body.is_null())
            .cloned())
    }
}

fn request_key(path: &str, query: &[(&str, &str)]) -> RequestKey {
    let mut query: Vec<(String, String)> = query
        .iter()
        .map(|&(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    query.sort();
    (path.to_owned(), query)
}

fn failure(error: TransportError) -> Error {
    Error::Transport(error)
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    /// Creates a scratch directory with one fixture file.
    fn scratch_directory(name: &str, file_name: &str, content: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("ringo-fixture-{}-{name}", process::id()));
        fs::create_dir_all(&directory).expect("scratch directory can be created");
        fs::write(directory.join(file_name), content).expect("fixture can be written");
        directory
    }

    #[tokio::test]
    async fn get_returns_the_json_of_the_file() {
        let directory = scratch_directory("known", "song.json", r#"{"data": [1]}"#);
        let transport = FixtureTransport::in_directory(&directory).with(
            "/v1/songs",
            &[("a", "1"), ("b", "2")],
            "song.json",
        );

        let value = transport.get("/v1/songs", &[("b", "2"), ("a", "1")]).await;

        fs::remove_dir_all(&directory).expect("scratch directory can be removed");
        assert_eq!(
            value.expect("request is known"),
            serde_json::json!({"data": [1]})
        );
    }

    #[tokio::test]
    async fn write_returns_the_response_body_and_records_the_request() {
        let transport = FixtureTransport::new().with_write(
            WriteMethod::Put,
            "/v1/me/ratings/songs/724515236",
            "write/rating_put_like.json",
        );
        let body = serde_json::json!({"type": "rating"});

        let response = transport
            .write(WriteRequest {
                method: WriteMethod::Put,
                path: "/v1/me/ratings/songs/724515236",
                query: &[("l", "en")],
                body: Some(&body),
            })
            .await
            .expect("request is known");

        let data = response.expect("response has a body");
        assert_eq!(data["data"][0]["type"], "ratings");
        assert_eq!(
            transport.recorded_writes(),
            vec![RecordedWrite {
                method: WriteMethod::Put,
                path: "/v1/me/ratings/songs/724515236".to_owned(),
                query: vec![("l".to_owned(), "en".to_owned())],
                body: Some(body),
            }]
        );
    }

    #[tokio::test]
    async fn write_returns_none_for_an_empty_response() {
        let transport = FixtureTransport::new().with_write(
            WriteMethod::Post,
            "/v1/me/library?ids[songs]=724515236",
            "write/library_add.json",
        );

        let response = transport
            .write(WriteRequest {
                method: WriteMethod::Post,
                path: "/v1/me/library?ids[songs]=724515236",
                query: &[],
                body: None,
            })
            .await;

        assert_eq!(response.expect("request is known"), None);
    }

    #[tokio::test]
    async fn write_returns_not_found_for_an_unknown_request() {
        let transport = FixtureTransport::in_directory("unused");

        let error = transport
            .write(WriteRequest {
                method: WriteMethod::Delete,
                path: "/v1/me/ratings/songs/1",
                query: &[],
                body: None,
            })
            .await
            .expect_err("request must fail");

        assert!(matches!(
            error,
            Error::Transport(TransportError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn get_returns_not_found_and_lists_the_request() {
        let transport = FixtureTransport::in_directory("unused");

        let error = transport
            .get("/v1/missing", &[("limit", "5")])
            .await
            .expect_err("request must fail");

        let Error::Transport(TransportError::NotFound { message }) = error else {
            panic!("expected NotFound, got {error:?}");
        };
        assert!(message.contains("/v1/missing"), "message was: {message}");
        assert!(message.contains("limit"), "message was: {message}");
    }

    #[tokio::test]
    async fn get_returns_other_for_a_missing_file() {
        let transport = FixtureTransport::in_directory("unused").with("/v1/a", &[], "none.json");

        let error = transport
            .get("/v1/a", &[])
            .await
            .expect_err("request must fail");

        assert!(matches!(
            error,
            Error::Transport(TransportError::Other { .. })
        ));
    }
}
