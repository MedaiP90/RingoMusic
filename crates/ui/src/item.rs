// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Plain data of the items that the views show. The widgets read these
//! values. They do not read the API models.

use std::time::Duration;

use ringo_am_api::models::{
    Album, Artist, Artwork, ContentRating, LibraryAlbum, LibraryArtist, LibraryPlaylist,
    LibrarySong, PlayParams, Playlist, Relationship, ResourceRef, Song,
};
use ringo_core::queue::QueueItem;
use ringo_engine::ItemId;

use crate::name_filter::NameFilter;

/// A song for a song row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongItem {
    /// The data for the queue. The song ID is the ID of the queue item.
    pub queue: QueueItem,
    /// The song has explicit content.
    pub explicit: bool,
    /// ID of the album, if the API sent it.
    pub album_id: Option<String>,
    /// ID of the main artist, if the API sent it.
    pub artist_id: Option<String>,
}

impl SongItem {
    /// The ID of the song.
    pub fn id(&self) -> &str {
        self.queue.id.as_str()
    }

    /// Tells if the title or the artist matches `filter`.
    pub fn matches(&self, filter: &NameFilter) -> bool {
        filter.matches(&[&self.queue.title, &self.queue.artist])
    }
}

impl From<&Song> for SongItem {
    fn from(song: &Song) -> Self {
        let attributes = &song.attributes;
        let id = playable_id(attributes.play_params.as_ref(), song.id.as_str());
        let relationships = song.relationships.as_ref();
        Self {
            queue: QueueItem {
                id: ItemId::new(id),
                title: attributes.name.clone(),
                artist: attributes.artist_name.clone().unwrap_or_default(),
                album: attributes.album_name.clone().unwrap_or_default(),
                duration: Duration::from_millis(u64::from(
                    attributes.duration_millis.unwrap_or_default(),
                )),
                artwork_template: template_of(attributes.artwork.as_ref()),
            },
            explicit: attributes.content_rating == Some(ContentRating::Explicit),
            album_id: relationships.and_then(|links| first_id(links.albums.as_ref())),
            artist_id: relationships.and_then(|links| first_id(links.artists.as_ref())),
        }
    }
}

impl From<&LibrarySong> for SongItem {
    /// A library song plays with its library ID. The API sends no links to
    /// the album or the artist, so the go-to entries stay off.
    fn from(song: &LibrarySong) -> Self {
        let attributes = &song.attributes;
        let id = playable_id(attributes.play_params.as_ref(), song.id.as_str());
        Self {
            queue: QueueItem {
                id: ItemId::new(id),
                title: attributes.name.clone(),
                artist: attributes.artist_name.clone().unwrap_or_default(),
                album: attributes.album_name.clone().unwrap_or_default(),
                duration: Duration::from_millis(u64::from(
                    attributes.duration_millis.unwrap_or_default(),
                )),
                artwork_template: template_of(attributes.artwork.as_ref()),
            },
            explicit: attributes.content_rating == Some(ContentRating::Explicit),
            album_id: None,
            artist_id: None,
        }
    }
}

/// The kind of item that a tile shows.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TileKind {
    Album,
    Artist,
    Playlist,
}

impl TileKind {
    /// The window action that opens an item of this kind.
    pub fn open_action(self) -> &'static str {
        match self {
            Self::Album => "win.open-album",
            Self::Artist => "win.open-artist",
            Self::Playlist => "win.open-playlist",
        }
    }
}

/// An album, an artist, or a playlist for a tile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileItem {
    pub kind: TileKind,
    /// ID that the open action gets.
    pub id: String,
    pub title: String,
    /// The artist of an album, or the curator of a playlist. Empty for an artist.
    pub subtitle: String,
    pub artwork_template: Option<String>,
}

impl TileItem {
    /// Tells if the title matches `filter`.
    pub fn matches(&self, filter: &NameFilter) -> bool {
        filter.matches(&[&self.title])
    }
}

impl From<&Album> for TileItem {
    fn from(album: &Album) -> Self {
        let attributes = &album.attributes;
        Self {
            kind: TileKind::Album,
            id: album.id.to_string(),
            title: attributes.name.clone(),
            subtitle: attributes.artist_name.clone().unwrap_or_default(),
            artwork_template: template_of(attributes.artwork.as_ref()),
        }
    }
}

impl From<&Artist> for TileItem {
    fn from(artist: &Artist) -> Self {
        Self {
            kind: TileKind::Artist,
            id: artist.id.to_string(),
            title: artist.attributes.name.clone(),
            subtitle: String::new(),
            artwork_template: template_of(artist.attributes.artwork.as_ref()),
        }
    }
}

impl From<&Playlist> for TileItem {
    fn from(playlist: &Playlist) -> Self {
        let attributes = &playlist.attributes;
        Self {
            kind: TileKind::Playlist,
            id: playlist.id.to_string(),
            title: attributes.name.clone(),
            subtitle: attributes.curator_name.clone().unwrap_or_default(),
            artwork_template: template_of(attributes.artwork.as_ref()),
        }
    }
}

impl From<&LibraryAlbum> for TileItem {
    fn from(album: &LibraryAlbum) -> Self {
        let attributes = &album.attributes;
        Self {
            kind: TileKind::Album,
            id: album.id.to_string(),
            title: attributes.name.clone(),
            subtitle: attributes.artist_name.clone().unwrap_or_default(),
            artwork_template: template_of(attributes.artwork.as_ref()),
        }
    }
}

impl From<&LibraryArtist> for TileItem {
    /// A library artist has no photo. The tile uses the photo of the linked
    /// catalog artist, if the request included it.
    fn from(artist: &LibraryArtist) -> Self {
        let artwork_template = artist
            .relationships
            .as_ref()
            .and_then(|links| links.catalog.as_ref())
            .and_then(|catalog| catalog.data.iter().next())
            .and_then(|catalog_artist| catalog_artist.attributes.artwork.as_ref())
            .map(|artwork| artwork.url.clone());
        Self {
            kind: TileKind::Artist,
            id: artist.id.to_string(),
            title: artist.attributes.name.clone(),
            subtitle: String::new(),
            artwork_template,
        }
    }
}

impl From<&LibraryPlaylist> for TileItem {
    fn from(playlist: &LibraryPlaylist) -> Self {
        let attributes = &playlist.attributes;
        Self {
            kind: TileKind::Playlist,
            id: playlist.id.to_string(),
            title: attributes.name.clone(),
            subtitle: String::new(),
            artwork_template: template_of(attributes.artwork.as_ref()),
        }
    }
}

/// The ID that the player needs: the one in the play parameters, if any.
fn playable_id(params: Option<&PlayParams>, fallback: &str) -> String {
    params.map_or_else(|| fallback.to_owned(), |params| params.id.clone())
}

fn template_of(artwork: Option<&Artwork>) -> Option<String> {
    artwork.map(|artwork| artwork.url.clone())
}

pub fn first_id(relationship: Option<&Relationship<ResourceRef>>) -> Option<String> {
    relationship?
        .data
        .first()
        .map(|resource| resource.id.clone())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn song(extra: serde_json::Value) -> Song {
        let mut value = json!({
            "id": "100",
            "type": "songs",
            "attributes": {
                "name": "Creep",
                "artistName": "Radiohead",
                "albumName": "Pablo Honey",
                "durationInMillis": 238_000,
                "artwork": { "url": "https://example.com/{w}x{h}.{f}" }
            }
        });
        value["relationships"] = extra;
        serde_json::from_value(value).expect("the test song is valid")
    }

    #[test]
    fn song_item_copies_the_attributes_into_the_queue_item() {
        let item = SongItem::from(&song(json!(null)));
        assert_eq!(item.id(), "100");
        assert_eq!(item.queue.title, "Creep");
        assert_eq!(item.queue.artist, "Radiohead");
        assert_eq!(item.queue.duration, Duration::from_secs(238));
        assert_eq!(
            item.queue.artwork_template.as_deref(),
            Some("https://example.com/{w}x{h}.{f}")
        );
        assert!(!item.explicit);
    }

    #[test]
    fn song_item_reads_the_album_and_artist_ids_from_the_relationships() {
        let links = json!({
            "albums": { "data": [{ "id": "7", "type": "albums" }] },
            "artists": { "data": [{ "id": "9", "type": "artists" }] }
        });
        let item = SongItem::from(&song(links));
        assert_eq!(item.album_id.as_deref(), Some("7"));
        assert_eq!(item.artist_id.as_deref(), Some("9"));
    }

    #[test]
    fn song_item_has_no_link_ids_without_relationships() {
        let item = SongItem::from(&song(json!(null)));
        assert_eq!((item.album_id, item.artist_id), (None, None));
    }

    #[test]
    fn library_song_item_plays_with_its_library_id() {
        let song: LibrarySong = serde_json::from_value(json!({
            "id": "i.abc",
            "type": "library-songs",
            "attributes": {
                "name": "Creep",
                "playParams": { "id": "i.abc", "kind": "song", "isLibrary": true }
            }
        }))
        .expect("the test song is valid");
        let item = SongItem::from(&song);
        assert_eq!(item.id(), "i.abc");
        assert_eq!((item.album_id, item.artist_id), (None, None));
    }

    #[test]
    fn tile_kinds_open_different_actions() {
        assert_eq!(TileKind::Album.open_action(), "win.open-album");
        assert_eq!(TileKind::Artist.open_action(), "win.open-artist");
        assert_eq!(TileKind::Playlist.open_action(), "win.open-playlist");
    }

    #[test]
    fn library_artist_tile_takes_the_photo_of_the_catalog_artist() {
        let with_catalog: LibraryArtist = serde_json::from_value(json!({
            "id": "r.1",
            "attributes": { "name": "Band" },
            "relationships": { "catalog": { "data": [{
                "id": "5",
                "attributes": { "name": "Band", "artwork": { "url": "photo/{w}x{h}bb.jpg" } }
            }] } }
        }))
        .expect("the test artist is valid");
        let without_catalog: LibraryArtist =
            serde_json::from_value(json!({ "id": "r.2", "attributes": { "name": "Solo" } }))
                .expect("the test artist is valid");

        let tile = TileItem::from(&with_catalog);

        assert_eq!(
            tile.artwork_template.as_deref(),
            Some("photo/{w}x{h}bb.jpg")
        );
        assert_eq!(tile.id, "r.1");
        assert_eq!(TileItem::from(&without_catalog).artwork_template, None);
    }
}
