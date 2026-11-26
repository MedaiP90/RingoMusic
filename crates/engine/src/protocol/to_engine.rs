// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::BTreeMap;

use crate::{Error, Result};

/// Method that Rust sends to the engine, with typed params.
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
#[serde(tag = "method", content = "params")]
pub enum ToEngine {
    /// Returns [`AuthStatus`].
    #[serde(rename = "auth.status")]
    AuthStatus,
    /// Shows the login window. Returns when the login completes or the user cancels.
    #[serde(rename = "auth.signIn")]
    AuthSignIn,
    /// Signs the user out.
    #[serde(rename = "auth.signOut")]
    AuthSignOut,
    /// Replaces the queue. Rust owns the order, including shuffle.
    #[serde(rename = "player.setQueue")]
    PlayerSetQueue {
        /// Catalog or library IDs, in play order.
        items: Vec<String>,
        /// Index in `items` of the first item to play.
        start_index: u32,
        /// Kind of the IDs in `items`.
        kind: QueueKind,
    },
    /// Inserts items after the current item. The current item keeps playing.
    #[serde(rename = "player.playNext")]
    PlayerPlayNext {
        /// Catalog or library IDs, in play order.
        items: Vec<String>,
        /// Kind of the IDs in `items`.
        kind: QueueKind,
    },
    /// Adds items at the end of the queue. The current item keeps playing.
    #[serde(rename = "player.playLater")]
    PlayerPlayLater {
        /// Catalog or library IDs, in play order.
        items: Vec<String>,
        /// Kind of the IDs in `items`.
        kind: QueueKind,
    },
    /// Removes the item at `index` from the queue. The current item cannot be removed.
    #[serde(rename = "player.remove")]
    PlayerRemove {
        /// Index in the queue that the engine plays.
        index: u32,
    },
    /// Starts or resumes playback.
    #[serde(rename = "player.play")]
    PlayerPlay,
    /// Pauses playback.
    #[serde(rename = "player.pause")]
    PlayerPause,
    /// Stops playback.
    #[serde(rename = "player.stop")]
    PlayerStop,
    /// Plays the next queue item.
    #[serde(rename = "player.next")]
    PlayerNext,
    /// Plays the previous queue item.
    #[serde(rename = "player.previous")]
    PlayerPrevious,
    /// Seeks in the current item.
    #[serde(rename = "player.seek")]
    PlayerSeek {
        /// New position in milliseconds.
        position_ms: u64,
    },
    /// Sets the volume.
    #[serde(rename = "player.setVolume")]
    PlayerSetVolume {
        /// Volume from 0.0 to 1.0.
        volume: f32,
    },
    /// Sets the repeat mode.
    #[serde(rename = "player.setRepeat")]
    PlayerSetRepeat {
        /// New repeat mode.
        mode: RepeatMode,
    },
    /// Runs a GET request with `api.music()`. Returns the raw JSON response.
    #[serde(rename = "api.get")]
    ApiGet {
        /// API path, for example `/v1/catalog/us/search`.
        path: String,
        /// Query parameters.
        query: BTreeMap<String, String>,
    },
    /// Runs a write request on the Apple Music API. Returns the raw JSON response,
    /// or `null` for an empty response. Rust checks the request with
    /// [`ApiWriteRequest::validate`] before it sends it.
    #[serde(rename = "api.write")]
    ApiWrite(ApiWriteRequest),
    /// Asks the engine to quit.
    #[serde(rename = "engine.shutdown")]
    EngineShutdown,
}

/// Kind of the IDs in a queue.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum QueueKind {
    /// Song IDs. Catalog IDs and library IDs (`i.…`) both work.
    Songs,
}

/// Repeat mode of the player.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// No repeat.
    None,
    /// Repeat the current item.
    One,
    /// Repeat the whole queue.
    All,
}

/// Result of `auth.status`.
#[derive(
    Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
pub struct AuthStatus {
    /// The user authorized the app.
    pub authorized: bool,
    /// Storefront ID, for example `us`.
    pub storefront: String,
}

/// Documented `DELETE` path patterns. A pattern is `(prefix, types)`: the path is
/// `prefix`, one of `types`, and an ID. Source:
/// `plan/04_library-management/apple-music-writes.md` (the ratings table).
const DELETE_PATTERNS: &[(&str, &[&str])] = &[(
    "/v1/me/ratings/",
    &[
        "songs",
        "albums",
        "playlists",
        "music-videos",
        "stations",
        "library-songs",
        "library-albums",
        "library-playlists",
        "library-music-videos",
    ],
)];

/// Path prefix that every write request must have.
const WRITE_PREFIX: &str = "/v1/me/";

/// HTTP method of a write request.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize,
)]
pub enum WriteMethod {
    /// Creates a resource or adds items.
    #[serde(rename = "POST")]
    Post,
    /// Sets a rating.
    #[serde(rename = "PUT")]
    Put,
    /// Removes a rating.
    #[serde(rename = "DELETE")]
    Delete,
}

/// Params of `api.write`.
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
pub struct ApiWriteRequest {
    /// HTTP method of the request.
    pub method: WriteMethod,
    /// API path. It starts with `/v1/me/`.
    pub path: String,
    /// Query parameters.
    pub query: BTreeMap<String, String>,
    /// JSON body, or none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

impl ApiWriteRequest {
    /// Checks the request before it goes to the engine.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParams`] if the path is not under `/v1/me/`,
    /// if it has a `.` or `..` segment, or if the method is `DELETE` and the
    /// path is not a documented delete path.
    pub fn validate(&self) -> Result<()> {
        let invalid = |detail: &str| Error::InvalidParams {
            detail: detail.to_owned(),
        };
        let Some(rest) = self.path.strip_prefix(WRITE_PREFIX) else {
            return Err(invalid("api.write path must start with /v1/me/"));
        };
        if rest.is_empty() || rest.split('/').any(|part| matches!(part, "" | "." | "..")) {
            return Err(invalid("api.write path has an empty or relative segment"));
        }
        if self.method == WriteMethod::Delete && !is_documented_delete(&self.path) {
            return Err(invalid("api.write DELETE is not allowed for this path"));
        }
        Ok(())
    }
}

/// Returns whether `path` matches a pattern of [`DELETE_PATTERNS`].
fn is_documented_delete(path: &str) -> bool {
    DELETE_PATTERNS.iter().any(|(prefix, types)| {
        path.strip_prefix(prefix)
            .and_then(|rest| rest.split_once('/'))
            .is_some_and(|(kind, id)| types.contains(&kind) && !id.contains('/'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: WriteMethod, path: &str) -> ApiWriteRequest {
        ApiWriteRequest {
            method,
            path: path.to_owned(),
            query: BTreeMap::new(),
            body: None,
        }
    }

    fn is_invalid_params(result: &Result<()>) -> bool {
        matches!(*result, Err(Error::InvalidParams { .. }))
    }

    #[test]
    fn validate_accepts_post_on_a_me_path() {
        assert!(
            request(WriteMethod::Post, "/v1/me/library")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn validate_accepts_put_on_a_rating_path() {
        let put = request(WriteMethod::Put, "/v1/me/ratings/songs/123");
        assert!(put.validate().is_ok());
    }

    #[test]
    fn validate_accepts_delete_on_a_rating_path() {
        let delete = request(WriteMethod::Delete, "/v1/me/ratings/library-albums/l.abc");
        assert!(delete.validate().is_ok());
    }

    #[test]
    fn validate_rejects_a_path_outside_me() {
        let result = request(WriteMethod::Post, "/v1/catalog/us/songs").validate();
        assert!(is_invalid_params(&result));
        assert!(is_invalid_params(
            &request(WriteMethod::Post, "/v1/me").validate()
        ));
    }

    #[test]
    fn validate_rejects_a_relative_segment() {
        let result = request(WriteMethod::Post, "/v1/me/../catalog/us").validate();
        assert!(is_invalid_params(&result));
    }

    #[test]
    fn validate_rejects_delete_on_an_undocumented_path() {
        for path in [
            "/v1/me/library/playlists/p.1",
            "/v1/me/ratings/artists/1",
            "/v1/me/ratings/songs",
            "/v1/me/ratings/songs/1/extra",
        ] {
            let result = request(WriteMethod::Delete, path).validate();
            assert!(is_invalid_params(&result), "{path}");
        }
    }
}
