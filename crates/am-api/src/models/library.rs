// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use super::id::{LibraryAlbumId, LibraryArtistId, LibraryPlaylistId, LibrarySongId};
use super::resource::Resource;
use super::shared::{Artwork, ContentRating, Description, PlayParams, Relationship, ResourceRef};

/// A song in the library of the user.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibrarySong {
    /// ID of the song.
    pub id: LibrarySongId,
    /// Attributes of the song.
    pub attributes: LibrarySongAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<LibrarySongRelationships>,
}

/// Resources that a library song links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibrarySongRelationships {
    /// Library albums of the song.
    pub albums: Option<Relationship<ResourceRef>>,
    /// Library artists of the song.
    pub artists: Option<Relationship<ResourceRef>>,
}

/// Attributes of a library song.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySongAttributes {
    /// Title of the song.
    pub name: String,
    /// Name of the artist.
    pub artist_name: Option<String>,
    /// Name of the album.
    pub album_name: Option<String>,
    /// Duration in milliseconds.
    #[serde(rename = "durationInMillis")]
    pub duration_millis: Option<u32>,
    /// Position of the song on its disc.
    pub track_number: Option<u32>,
    /// Number of the disc.
    pub disc_number: Option<u32>,
    /// Release date. Apple can send only the year.
    pub release_date: Option<String>,
    /// Cover image.
    pub artwork: Option<Artwork>,
    /// Content rating.
    pub content_rating: Option<ContentRating>,
    /// Data for the player.
    pub play_params: Option<PlayParams>,
}

/// An album in the library of the user.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibraryAlbum {
    /// ID of the album.
    pub id: LibraryAlbumId,
    /// Attributes of the album.
    pub attributes: LibraryAlbumAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<LibraryAlbumRelationships>,
}

/// Attributes of a library album.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAlbumAttributes {
    /// Title of the album.
    pub name: String,
    /// Name of the artist.
    pub artist_name: Option<String>,
    /// Release date. Apple can send only the year.
    pub release_date: Option<String>,
    /// Number of tracks.
    pub track_count: Option<u32>,
    /// Cover image.
    pub artwork: Option<Artwork>,
    /// Content rating.
    pub content_rating: Option<ContentRating>,
    /// Data for the player.
    pub play_params: Option<PlayParams>,
}

/// Resources that a library album links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibraryAlbumRelationships {
    /// Tracks of the album.
    pub tracks: Option<Relationship<Resource>>,
    /// Artists of the album. Only a request that includes them has this.
    pub artists: Option<Relationship<ResourceRef>>,
}

/// An artist in the library of the user.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibraryArtist {
    /// ID of the artist.
    pub id: LibraryArtistId,
    /// Attributes of the artist.
    pub attributes: LibraryArtistAttributes,
}

/// Attributes of a library artist.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct LibraryArtistAttributes {
    /// Name of the artist.
    pub name: String,
}

/// A playlist in the library of the user.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibraryPlaylist {
    /// ID of the playlist.
    pub id: LibraryPlaylistId,
    /// Attributes of the playlist.
    pub attributes: LibraryPlaylistAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<LibraryPlaylistRelationships>,
}

/// Attributes of a library playlist.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaylistAttributes {
    /// Title of the playlist.
    pub name: String,
    /// Text about the playlist.
    pub description: Option<Description>,
    /// Date of the last change.
    pub last_modified_date: Option<String>,
    /// Cover image.
    pub artwork: Option<Artwork>,
    /// Data for the player.
    pub play_params: Option<PlayParams>,
}

/// Resources that a library playlist links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct LibraryPlaylistRelationships {
    /// Tracks of the playlist.
    pub tracks: Option<Relationship<Resource>>,
}
