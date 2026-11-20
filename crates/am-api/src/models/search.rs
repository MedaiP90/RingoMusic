// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use super::catalog::{Album, Artist, Playlist, Song};
use super::library::{LibraryAlbum, LibraryArtist, LibraryPlaylist, LibrarySong};
use super::resource::Resource;
use super::shared::ApiPage;

/// Results of a catalog search. Apple sends one list for each requested type.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct SearchResults {
    /// Matching songs.
    pub songs: Option<ApiPage<Song>>,
    /// Matching albums.
    pub albums: Option<ApiPage<Album>>,
    /// Matching artists.
    pub artists: Option<ApiPage<Artist>>,
    /// Matching playlists.
    pub playlists: Option<ApiPage<Playlist>>,
    /// Best matches of any type. Present only if the request used `with=topResults`.
    #[serde(rename = "topResults")]
    pub top_results: Option<ApiPage<Resource>>,
}

/// Results of a library search. Apple sends one list for each requested type.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct LibrarySearchResults {
    /// Matching songs.
    #[serde(rename = "library-songs")]
    pub songs: Option<ApiPage<LibrarySong>>,
    /// Matching albums.
    #[serde(rename = "library-albums")]
    pub albums: Option<ApiPage<LibraryAlbum>>,
    /// Matching artists.
    #[serde(rename = "library-artists")]
    pub artists: Option<ApiPage<LibraryArtist>>,
    /// Matching playlists.
    #[serde(rename = "library-playlists")]
    pub playlists: Option<ApiPage<LibraryPlaylist>>,
}
