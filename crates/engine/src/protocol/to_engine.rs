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

/// The only write requests that the app sends. Each one is documented by Apple
/// (`plan/04_library-management/apple-music-writes.md`) and used by `ringo-am-api::write`.
/// CONTRIBUTING.md rule 3: add a pattern only for a documented endpoint.
///
/// In a pattern, `{id}` is one ID segment (see [`is_id_segment`]) and `a|b` is one of
/// the literal segments `a` and `b`.
///
/// Keep this list equal to `WRITE_PATTERNS` in `engine/src/api-write.ts`.
const WRITE_PATTERNS: &[(WriteMethod, &str)] = &[
    (WriteMethod::Post, "/v1/me/library"),
    (WriteMethod::Post, "/v1/me/library/playlists"),
    (WriteMethod::Post, "/v1/me/library/playlists/{id}/tracks"),
    (WriteMethod::Post, "/v1/me/favorites"),
    (WriteMethod::Put, "/v1/me/ratings/songs/{id}"),
    (
        WriteMethod::Delete,
        "/v1/me/ratings/songs|albums|playlists/{id}",
    ),
];

/// Placeholder for one ID segment in [`WRITE_PATTERNS`].
const ID_PLACEHOLDER: &str = "{id}";

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
    /// Returns [`Error::InvalidParams`] if the method and the path do not match a pattern
    /// of [`WRITE_PATTERNS`].
    pub fn validate(&self) -> Result<()> {
        let is_permitted = WRITE_PATTERNS.iter().any(|&(method, pattern)| {
            method == self.method && matches_pattern(&self.path, pattern)
        });
        if !is_permitted {
            return Err(Error::InvalidParams {
                detail: "api.write is not allowed for this method and path".to_owned(),
            });
        }
        Ok(())
    }
}

/// Returns whether `path` matches `pattern`, one segment at a time.
fn matches_pattern(path: &str, pattern: &str) -> bool {
    let path_segments: Vec<&str> = path.split('/').collect();
    let pattern_segments: Vec<&str> = pattern.split('/').collect();
    path_segments.len() == pattern_segments.len()
        && path_segments
            .iter()
            .zip(&pattern_segments)
            .all(|(&segment, &expected)| matches_segment(segment, expected))
}

fn matches_segment(segment: &str, expected: &str) -> bool {
    if expected == ID_PLACEHOLDER {
        is_id_segment(segment)
    } else {
        expected.split('|').any(|literal| literal == segment)
    }
}

/// Returns whether `segment` is a safe ID: ASCII letters, digits, `.`, `_`, and `-`, and
/// not a relative segment. The same rule as `segment()` in `ringo-am-api`. Other
/// characters can change the path when the engine builds the URL (`%2e`, `\`, a tab).
fn is_id_segment(segment: &str) -> bool {
    let is_permitted =
        |character: char| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
    !matches!(segment, "" | "." | "..") && segment.chars().all(is_permitted)
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
    fn validate_accepts_each_permitted_write() {
        let permitted = [
            (WriteMethod::Post, "/v1/me/library"),
            (WriteMethod::Post, "/v1/me/library/playlists"),
            (
                WriteMethod::Post,
                "/v1/me/library/playlists/p.TestPlaylist0001/tracks",
            ),
            (WriteMethod::Post, "/v1/me/favorites"),
            (WriteMethod::Put, "/v1/me/ratings/songs/123"),
            (WriteMethod::Delete, "/v1/me/ratings/songs/123"),
            (WriteMethod::Delete, "/v1/me/ratings/albums/456"),
            (WriteMethod::Delete, "/v1/me/ratings/playlists/pl.u-abc_1"),
        ];
        for (method, path) in permitted {
            assert!(
                request(method, path).validate().is_ok(),
                "{method:?} {path}"
            );
        }
    }

    #[test]
    fn validate_rejects_a_path_outside_the_patterns() {
        let rejected = [
            (WriteMethod::Post, "/v1/catalog/us/songs"),
            (WriteMethod::Post, "/v1/me"),
            (WriteMethod::Post, "/v1/me/"),
            (WriteMethod::Post, "/v1/me/library/"),
            (WriteMethod::Post, "/v1/me/library/playlist-folders"),
            (WriteMethod::Post, "/v1/me/library?ids[songs]=1"),
            (WriteMethod::Delete, "/v1/me/ratings/songs"),
            (WriteMethod::Delete, "/v1/me/ratings/songs/1/extra"),
        ];
        for (method, path) in rejected {
            let result = request(method, path).validate();
            assert!(is_invalid_params(&result), "{method:?} {path}");
        }
    }

    #[test]
    fn validate_rejects_a_method_that_the_pattern_does_not_permit() {
        let rejected = [
            (WriteMethod::Put, "/v1/me/library"),
            (WriteMethod::Put, "/v1/me/ratings/albums/1"),
            (WriteMethod::Put, "/v1/me/ratings/artists/1"),
            (WriteMethod::Delete, "/v1/me/ratings/artists/1"),
            (WriteMethod::Delete, "/v1/me/ratings/library-songs/i.1"),
            (WriteMethod::Delete, "/v1/me/library/playlists/p.1"),
        ];
        for (method, path) in rejected {
            let result = request(method, path).validate();
            assert!(is_invalid_params(&result), "{method:?} {path}");
        }
    }

    #[test]
    fn validate_rejects_an_id_that_can_change_the_path() {
        let ids = ["", ".", "..", "%2e%2e", "..\\..", "a\tb", "a b"];
        for id in ids {
            let path = format!("/v1/me/ratings/songs/{id}");
            let result = request(WriteMethod::Delete, &path).validate();
            assert!(is_invalid_params(&result), "{path:?}");
        }
    }
}
