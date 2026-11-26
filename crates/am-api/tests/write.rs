// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tests the write functions with the recorded API responses.

#![cfg(feature = "write")]
#![expect(clippy::unwrap_used, reason = "tests may panic")]

use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use ringo_am_api::models::{AlbumId, ArtistId, LibraryPlaylistId, PlaylistId, SongId};
use ringo_am_api::write::{self, CatalogItem, Rating, Track};
use ringo_am_api::{
    ApiTransport, Error, FixtureTransport, RecordedWrite, TransportError, WriteMethod, WriteRequest,
};

const STOREFRONT: &str = "it";
const SONG: &str = "724515236";
const ALBUM: &str = "724514728";
const PLAYLIST_ID: &str = "p.TestPlaylist0001";

fn song() -> CatalogItem {
    CatalogItem::Song(SongId::from(SONG.to_owned()))
}

fn album() -> CatalogItem {
    CatalogItem::Album(AlbumId::from(ALBUM.to_owned()))
}

fn artist() -> CatalogItem {
    CatalogItem::Artist(ArtistId::from("1".to_owned()))
}

fn track(id: &str) -> Track {
    Track::Song(SongId::from(id.to_owned()))
}

fn playlist_id() -> LibraryPlaylistId {
    LibraryPlaylistId::from(PLAYLIST_ID.to_owned())
}

fn query(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|&(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

/// Counts the read calls, so a test can tell `get` from `get_uncached`.
struct SpyTransport {
    inner: FixtureTransport,
    cached: AtomicUsize,
    uncached: AtomicUsize,
}

impl SpyTransport {
    fn new(inner: FixtureTransport) -> Self {
        Self {
            inner,
            cached: AtomicUsize::new(0),
            uncached: AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl ApiTransport for SpyTransport {
    async fn get(&self, path: &str, query: &[(&str, &str)]) -> ringo_am_api::Result<Value> {
        self.cached.fetch_add(1, Ordering::SeqCst);
        self.inner.get(path, query).await
    }

    async fn get_uncached(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> ringo_am_api::Result<Value> {
        self.uncached.fetch_add(1, Ordering::SeqCst);
        self.inner.get_uncached(path, query).await
    }

    async fn write(&self, request: WriteRequest<'_>) -> ringo_am_api::Result<Option<Value>> {
        self.inner.write(request).await
    }
}

#[tokio::test]
async fn create_playlist_sends_the_body_and_parses_the_playlist() {
    let transport = FixtureTransport::new().with_write(
        WriteMethod::Post,
        "/v1/me/library/playlists",
        "write/playlist_create.json",
    );

    let playlist = write::create_playlist(&transport, " RingoMusic Test ", Some("About"), &[])
        .await
        .unwrap();

    assert_eq!(playlist.id.as_str(), PLAYLIST_ID);
    assert_eq!(playlist.attributes.name, "RingoMusic Test");
    assert!(playlist.attributes.can_edit);
    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Post,
            path: "/v1/me/library/playlists".to_owned(),
            query: Vec::new(),
            body: Some(
                json!({ "attributes": { "name": "RingoMusic Test", "description": "About" } })
            ),
        }]
    );
}

#[tokio::test]
async fn create_playlist_sends_the_tracks_in_order() {
    let transport = FixtureTransport::new().with_write(
        WriteMethod::Post,
        "/v1/me/library/playlists",
        "write/playlist_create.json",
    );

    write::create_playlist(&transport, "Mix", None, &[track("2"), track("1")])
        .await
        .unwrap();

    let body = transport.recorded_writes().remove(0).body.unwrap();
    assert_eq!(
        body["relationships"]["tracks"]["data"],
        json!([{ "id": "2", "type": "songs" }, { "id": "1", "type": "songs" }])
    );
}

#[tokio::test]
async fn create_playlist_rejects_an_empty_name_without_a_request() {
    let transport = FixtureTransport::new();

    let error = write::create_playlist(&transport, "  ", None, &[])
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
    assert_eq!(transport.recorded_writes(), Vec::new());
}

#[tokio::test]
async fn add_to_playlist_sends_one_request_for_all_tracks() {
    let path = format!("/v1/me/library/playlists/{PLAYLIST_ID}/tracks");
    let transport = FixtureTransport::new().with_write(
        WriteMethod::Post,
        &path,
        "write/playlist_add_tracks.json",
    );
    let tracks = [track("3"), track("1"), track("2")];

    write::add_to_playlist(&transport, &playlist_id(), &tracks)
        .await
        .unwrap();

    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Post,
            path,
            query: Vec::new(),
            body: Some(json!({ "data": [
                { "id": "3", "type": "songs" },
                { "id": "1", "type": "songs" },
                { "id": "2", "type": "songs" },
            ] })),
        }]
    );
}

#[tokio::test]
async fn add_to_playlist_rejects_no_tracks_without_a_request() {
    let transport = FixtureTransport::new();

    let error = write::add_to_playlist(&transport, &playlist_id(), &[])
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
    assert_eq!(transport.recorded_writes(), Vec::new());
}

#[tokio::test]
async fn add_to_library_sends_several_types_in_one_request() {
    let transport = FixtureTransport::new().with_write(
        WriteMethod::Post,
        "/v1/me/library",
        "write/library_add.json",
    );
    let items = [
        song(),
        album(),
        CatalogItem::Song(SongId::from("5".to_owned())),
        CatalogItem::Playlist(PlaylistId::from("pl.1".to_owned())),
    ];

    write::add_to_library(&transport, &items).await.unwrap();

    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Post,
            path: "/v1/me/library".to_owned(),
            query: query(&[
                ("ids[songs]", "724515236,5"),
                ("ids[albums]", ALBUM),
                ("ids[playlists]", "pl.1"),
            ]),
            body: None,
        }]
    );
}

#[tokio::test]
async fn add_to_library_rejects_an_artist_without_a_request() {
    let transport = FixtureTransport::new();

    let error = write::add_to_library(&transport, &[song(), artist()])
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
    assert_eq!(transport.recorded_writes(), Vec::new());
}

#[tokio::test]
async fn add_to_favorites_sends_one_type_in_the_key() {
    let transport = FixtureTransport::new().with_write(
        WriteMethod::Post,
        "/v1/me/favorites",
        "write/favorites_add.json",
    );

    write::add_to_favorites(&transport, &artist())
        .await
        .unwrap();

    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Post,
            path: "/v1/me/favorites".to_owned(),
            query: query(&[("ids[artists]", "1")]),
            body: None,
        }]
    );
}

#[tokio::test]
async fn remove_from_favorites_deletes_the_rating_of_a_song() {
    let path = format!("/v1/me/ratings/songs/{SONG}");
    let transport =
        FixtureTransport::new().with_write(WriteMethod::Delete, &path, "write/rating_delete.json");

    write::remove_from_favorites(&transport, &song())
        .await
        .unwrap();

    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Delete,
            path,
            query: Vec::new(),
            body: None,
        }]
    );
}

#[tokio::test]
async fn remove_from_favorites_puts_a_dislike_for_an_artist() {
    let transport = FixtureTransport::new().with_write(
        WriteMethod::Put,
        "/v1/me/ratings/artists/1",
        "write/rating_put_dislike.json",
    );

    write::remove_from_favorites(&transport, &artist())
        .await
        .unwrap();

    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Put,
            path: "/v1/me/ratings/artists/1".to_owned(),
            query: Vec::new(),
            body: Some(json!({ "type": "rating", "attributes": { "value": -1 } })),
        }]
    );
}

#[tokio::test]
async fn favorite_status_reads_uncached_and_covers_every_item() {
    let other_song = CatalogItem::Song(SongId::from("724515237".to_owned()));
    let fixtures = FixtureTransport::new()
        .with(
            "/v1/catalog/it/songs",
            &[("ids", "724515236,724515237"), ("extend", "inFavorites")],
            "write/read_favorites_songs.json",
        )
        .with(
            "/v1/catalog/it/albums",
            &[("ids", ALBUM), ("extend", "inFavorites")],
            "write/read_favorites_albums.json",
        );
    let transport = SpyTransport::new(fixtures);

    let status = write::favorite_status(
        &transport,
        STOREFRONT,
        &[song(), album(), other_song.clone()],
    )
    .await
    .unwrap();

    assert_eq!(status.len(), 3);
    assert!(status[&song()]);
    assert!(status[&album()]);
    assert!(!status[&other_song]);
    assert_eq!(transport.uncached.load(Ordering::SeqCst), 2);
    assert_eq!(transport.cached.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dislike_status_marks_only_the_songs_with_a_dislike() {
    let unrated = CatalogItem::Song(SongId::from("1".to_owned()));
    let loved = CatalogItem::Song(SongId::from("724515237".to_owned()));
    let transport = FixtureTransport::new().with(
        "/v1/me/ratings/songs",
        &[("ids", "724515236,724515237,1")],
        "write/read_ratings_songs.json",
    );

    let status = write::dislike_status(
        &transport,
        &[song(), loved.clone(), unrated.clone(), album()],
    )
    .await
    .unwrap();

    assert_eq!(status.len(), 3);
    assert!(status[&song()]);
    assert!(!status[&loved]);
    assert!(!status[&unrated]);
}

#[tokio::test]
async fn rating_reads_the_value() {
    let transport = FixtureTransport::new().with(
        &format!("/v1/me/ratings/songs/{SONG}"),
        &[],
        "write/read_rating_song.json",
    );

    let rating = write::rating(&transport, &song()).await.unwrap();

    assert_eq!(rating, Some(Rating::Love));
}

#[tokio::test]
async fn rating_returns_none_when_apple_has_no_rating() {
    // An unknown request is `NotFound`, like the `404` of Apple.
    let transport = FixtureTransport::in_directory("unused");

    let rating = write::rating(&transport, &song()).await.unwrap();

    assert_eq!(rating, None);
}

#[tokio::test]
async fn rating_returns_none_for_an_album_without_a_request() {
    let transport = SpyTransport::new(FixtureTransport::in_directory("unused"));

    let rating = write::rating(&transport, &album()).await.unwrap();

    assert_eq!(rating, None);
    assert_eq!(transport.cached.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn rating_passes_other_errors_on() {
    let transport = FixtureTransport::in_directory("unused").with(
        &format!("/v1/me/ratings/songs/{SONG}"),
        &[],
        "missing.json",
    );

    let error = write::rating(&transport, &song()).await.unwrap_err();

    assert!(
        matches!(error, Error::Transport(TransportError::Other { .. })),
        "got {error:?}"
    );
}

#[tokio::test]
async fn set_rating_puts_the_value() {
    let path = format!("/v1/me/ratings/songs/{SONG}");
    let transport =
        FixtureTransport::new().with_write(WriteMethod::Put, &path, "write/rating_put_like.json");

    write::set_rating(&transport, &song(), Rating::Love)
        .await
        .unwrap();

    assert_eq!(
        transport.recorded_writes(),
        vec![RecordedWrite {
            method: WriteMethod::Put,
            path,
            query: Vec::new(),
            body: Some(json!({ "type": "rating", "attributes": { "value": 1 } })),
        }]
    );
}

#[tokio::test]
async fn set_rating_rejects_an_artist_without_a_request() {
    let transport = FixtureTransport::new();

    let error = write::set_rating(&transport, &artist(), Rating::Dislike)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
    assert_eq!(transport.recorded_writes(), Vec::new());
}

#[tokio::test]
async fn remove_rating_deletes_the_rating_of_an_album() {
    let path = format!("/v1/me/ratings/albums/{ALBUM}");
    let transport =
        FixtureTransport::new().with_write(WriteMethod::Delete, &path, "write/rating_delete.json");

    write::remove_rating(&transport, &album()).await.unwrap();

    assert_eq!(transport.recorded_writes()[0].path, path);
}

#[tokio::test]
async fn library_status_is_true_when_the_library_relationship_has_data() {
    let fixtures = FixtureTransport::new().with(
        &format!("/v1/catalog/it/albums/{ALBUM}"),
        &[("include", "library")],
        "write/read_library_album_in_library.json",
    );
    let transport = SpyTransport::new(fixtures);

    let in_library = write::library_status(&transport, STOREFRONT, &album())
        .await
        .unwrap();

    assert!(in_library);
    assert_eq!(transport.uncached.load(Ordering::SeqCst), 1);
    assert_eq!(transport.cached.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn library_status_is_false_when_the_library_relationship_is_empty() {
    let transport = FixtureTransport::new().with(
        &format!("/v1/catalog/it/songs/{SONG}"),
        &[("include", "library")],
        "write/read_library_song_not_in_library.json",
    );

    let in_library = write::library_status(&transport, STOREFRONT, &song())
        .await
        .unwrap();

    assert!(!in_library);
}

#[tokio::test]
async fn a_write_without_a_response_fails_with_the_transport_error() {
    let transport = FixtureTransport::in_directory("unused");

    let error = write::add_to_favorites(&transport, &song())
        .await
        .unwrap_err();

    assert!(
        matches!(error, Error::Transport(TransportError::NotFound { .. })),
        "got {error:?}"
    );
}
