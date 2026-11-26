// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use serde_json::Value;

use crate::Result;

/// HTTP method of a write request.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum WriteMethod {
    /// Creates a resource or adds items.
    Post,
    /// Sets a rating.
    Put,
    /// Removes a rating.
    Delete,
}

/// Write request to the Apple Music API.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WriteRequest<'a> {
    /// HTTP method of the request.
    pub method: WriteMethod,
    /// API path. It starts with `/v1/me/`.
    pub path: &'a str,
    /// Query parameters.
    pub query: &'a [(&'a str, &'a str)],
    /// JSON body, or none.
    pub body: Option<&'a Value>,
}

/// Way to send requests to the Apple Music API.
///
/// Each implementation returns the raw JSON of the response.
#[async_trait::async_trait]
pub trait ApiTransport: Send + Sync {
    /// Sends a GET request. `path` starts with `/v1/`. `query` holds the query parameters.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transport`](crate::Error::Transport) if the request fails.
    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value>;

    /// Sends a GET request and never reads or writes the cache.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transport`](crate::Error::Transport) if the request fails.
    async fn get_uncached(&self, path: &str, query: &[(&str, &str)]) -> Result<Value>;

    /// Sends a write request. `path` starts with `/v1/me/`.
    /// Returns `None` if the response is empty.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transport`](crate::Error::Transport) if the request fails.
    async fn write(&self, request: WriteRequest<'_>) -> Result<Option<Value>>;
}
