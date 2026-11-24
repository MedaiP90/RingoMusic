// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use super::id::{AlbumId, ArtistId, PlaylistId, SongId};
use super::resource::Resource;
use super::shared::{
    ApiPage, Artwork, ContentRating, Description, PlayParams, Relationship, ResourceRef,
};

/// A song of the Apple Music catalog.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Song {
    /// ID of the song.
    pub id: SongId,
    /// Attributes of the song.
    pub attributes: SongAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<SongRelationships>,
}

/// Attributes of a catalog song.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongAttributes {
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

/// Resources that a catalog song links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct SongRelationships {
    /// Albums of the song.
    pub albums: Option<Relationship<ResourceRef>>,
    /// Artists of the song.
    pub artists: Option<Relationship<ResourceRef>>,
}

/// An album of the Apple Music catalog.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Album {
    /// ID of the album.
    pub id: AlbumId,
    /// Attributes of the album.
    pub attributes: AlbumAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<AlbumRelationships>,
}

/// Attributes of a catalog album.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumAttributes {
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

/// Resources that a catalog album links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct AlbumRelationships {
    /// Tracks of the album. Full resources if the request used `include=tracks`.
    pub tracks: Option<Relationship<Resource>>,
    /// Artists of the album.
    pub artists: Option<Relationship<ResourceRef>>,
}

/// An artist of the Apple Music catalog.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Artist {
    /// ID of the artist.
    pub id: ArtistId,
    /// Attributes of the artist.
    pub attributes: ArtistAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<ArtistRelationships>,
    /// Views of the artist, if the request used `views=...`.
    pub views: Option<ArtistViews>,
}

/// Attributes of a catalog artist. The fields after `genre_names` are
/// present only if the request used `extend=...`.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistAttributes {
    /// Name of the artist.
    pub name: String,
    /// Photo of the artist.
    pub artwork: Option<Artwork>,
    /// Genres of the artist, in the language of the storefront.
    #[serde(default)]
    pub genre_names: Vec<String>,
    /// Biography of the artist. It can contain HTML tags such as `<i>`.
    pub artist_bio: Option<String>,
    /// Birth date of a person or start year of a group, as a full date or a
    /// year.
    pub born_or_formed: Option<String>,
    /// Place where the artist comes from.
    pub origin: Option<String>,
    /// True for a group, false for a person.
    pub is_group: Option<bool>,
}

/// Resources that a catalog artist links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct ArtistRelationships {
    /// Albums of the artist.
    pub albums: Option<Relationship<ResourceRef>>,
}

/// Views of a catalog artist.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct ArtistViews {
    /// Most played songs of the artist.
    #[serde(rename = "top-songs")]
    pub top_songs: Option<ApiPage<Song>>,
    /// Full albums of the artist.
    #[serde(rename = "full-albums")]
    pub full_albums: Option<ApiPage<Album>>,
    /// Artists that Apple Music finds similar to the artist.
    #[serde(rename = "similar-artists")]
    pub similar_artists: Option<ApiPage<Artist>>,
}

/// A playlist of the Apple Music catalog.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Playlist {
    /// ID of the playlist.
    pub id: PlaylistId,
    /// Attributes of the playlist.
    pub attributes: PlaylistAttributes,
    /// Related resources, if the request asked for them.
    pub relationships: Option<PlaylistRelationships>,
}

/// Attributes of a catalog playlist.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistAttributes {
    /// Title of the playlist.
    pub name: String,
    /// Name of the curator.
    pub curator_name: Option<String>,
    /// Text about the playlist.
    pub description: Option<Description>,
    /// Date of the last change.
    pub last_modified_date: Option<String>,
    /// Cover image.
    pub artwork: Option<Artwork>,
    /// Data for the player.
    pub play_params: Option<PlayParams>,
}

/// Resources that a catalog playlist links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct PlaylistRelationships {
    /// Tracks of the playlist. Full resources if the request used `include=tracks`.
    pub tracks: Option<Relationship<Resource>>,
}
