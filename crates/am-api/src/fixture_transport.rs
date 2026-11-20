// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{ApiTransport, Error, Result, TransportError};

/// Request key: the path and the sorted query parameters.
type RequestKey = (String, Vec<(String, String)>);

/// [`ApiTransport`] that answers from recorded JSON files. It is for tests.
///
/// Each entry maps one request to one file in the fixture directory. A request
/// without an entry fails with [`TransportError::NotFound`].
#[derive(Debug)]
pub struct FixtureTransport {
    directory: PathBuf,
    entries: HashMap<RequestKey, String>,
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
