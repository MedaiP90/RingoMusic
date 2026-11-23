// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use serde_json::Value;

use crate::Result;

/// Way to send GET requests to the Apple Music API.
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
}
