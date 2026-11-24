// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Serde models for the Apple Music API resources.
//!
//! The models keep only the fields that the UI and the queue need. A list of
//! resources uses [`LenientVec`], so one broken item never breaks a response.

mod catalog;
mod grouping;
mod id;
mod lenient;
mod library;
mod recommendation;
mod resource;
mod resource_map;
mod search;
mod shared;

pub use catalog::{
    Album, AlbumAttributes, AlbumRelationships, Artist, ArtistAttributes, ArtistRelationships,
    ArtistViews, Playlist, PlaylistAttributes, PlaylistRelationships, Song, SongAttributes,
    SongRelationships,
};
pub use grouping::{Group, GroupKind};
pub use id::{
    AlbumId, ArtistId, LibraryAlbumId, LibraryArtistId, LibraryPlaylistId, LibrarySongId,
    PlaylistId, SongId,
};
pub use lenient::LenientVec;
pub use library::{
    LibraryAlbum, LibraryAlbumAttributes, LibraryAlbumRelationships, LibraryArtist,
    LibraryArtistAttributes, LibraryPlaylist, LibraryPlaylistAttributes,
    LibraryPlaylistRelationships, LibrarySong, LibrarySongAttributes, LibrarySongRelationships,
};
pub use recommendation::Recommendation;
pub use resource::Resource;
pub use search::{LibrarySearchResults, SearchResults};
pub use shared::{
    ApiPage, Artwork, ContentRating, Description, PlayParams, Relationship, ResourceRef,
};

pub(crate) use grouping::MapResponse;
