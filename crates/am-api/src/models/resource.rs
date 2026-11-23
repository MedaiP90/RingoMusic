// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use super::catalog::{Album, Artist, Playlist, Song};
use super::library::{LibraryAlbum, LibraryArtist, LibraryPlaylist, LibrarySong};

/// A resource of any known type. Lists of mixed resources use this type, for
/// example track lists and recommendations. The `type` field selects the variant.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Resource {
    /// A catalog song.
    #[serde(rename = "songs")]
    Song(Song),
    /// A catalog album.
    #[serde(rename = "albums")]
    Album(Album),
    /// A catalog artist.
    #[serde(rename = "artists")]
    Artist(Artist),
    /// A catalog playlist.
    #[serde(rename = "playlists")]
    Playlist(Playlist),
    /// A song in the library.
    #[serde(rename = "library-songs")]
    LibrarySong(LibrarySong),
    /// An album in the library.
    #[serde(rename = "library-albums")]
    LibraryAlbum(LibraryAlbum),
    /// An artist in the library.
    #[serde(rename = "library-artists")]
    LibraryArtist(LibraryArtist),
    /// A playlist in the library.
    #[serde(rename = "library-playlists")]
    LibraryPlaylist(LibraryPlaylist),
    /// A type that this app does not model, for example a station.
    #[serde(other)]
    Unknown,
}
