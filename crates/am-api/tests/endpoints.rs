// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tests the endpoint functions with the recorded API responses.

#![expect(clippy::unwrap_used, reason = "tests may panic")]

use ringo_am_api::endpoints::{self, CatalogSearchType, LibrarySearchType};
use ringo_am_api::models::{AlbumId, ArtistId, LibraryPlaylistId, PlaylistId, Resource, SongId};
use ringo_am_api::{Error, FixtureTransport};

const STOREFRONT: &str = "it";
const PLAYLIST_ID: &str = "pl.f4d106fed2bd41149aaacabb233eb5eb";
const PAGE_QUERY: [(&str, &str); 2] = [("limit", "25"), ("offset", "0")];

fn catalog(kind: &str) -> String {
    format!("/v1/catalog/{STOREFRONT}/{kind}")
}

#[tokio::test]
async fn search_returns_results_with_top_results() {
    let transport = FixtureTransport::new().with(
        &catalog("search"),
        &[
            ("term", "radiohead"),
            ("types", "songs,albums,artists,playlists"),
            ("limit", "5"),
            ("with", "topResults"),
        ],
        "catalog_search_top.json",
    );
    let types = [
        CatalogSearchType::Songs,
        CatalogSearchType::Albums,
        CatalogSearchType::Artists,
        CatalogSearchType::Playlists,
    ];

    let results = endpoints::search(&transport, STOREFRONT, " radiohead ", &types, 5)
        .await
        .unwrap();

    assert!(!results.songs.unwrap().data.is_empty());
    assert!(!results.top_results.unwrap().data.is_empty());
}

#[tokio::test]
async fn search_rejects_an_empty_term_without_a_request() {
    let transport = FixtureTransport::new();

    let error = endpoints::search(&transport, STOREFRONT, "  ", &[CatalogSearchType::Songs], 5)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
}

#[tokio::test]
async fn search_rejects_empty_types_without_a_request() {
    let transport = FixtureTransport::new();

    let error = endpoints::search(&transport, STOREFRONT, "radiohead", &[], 5)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
}

#[tokio::test]
async fn library_search_returns_results() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/search",
        &[
            ("term", "example"),
            ("types", "library-songs,library-artists"),
            ("limit", "5"),
        ],
        "library_search.json",
    );
    let types = [LibrarySearchType::Songs, LibrarySearchType::Artists];

    let results = endpoints::library_search(&transport, "example", &types, 5)
        .await
        .unwrap();

    assert!(!results.songs.unwrap().data.is_empty());
    assert!(!results.artists.unwrap().data.is_empty());
}

#[tokio::test]
async fn library_search_rejects_an_empty_term_without_a_request() {
    let transport = FixtureTransport::new();

    let error = endpoints::library_search(&transport, "", &[LibrarySearchType::Songs], 5)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
}

#[tokio::test]
async fn song_returns_the_song() {
    let transport =
        FixtureTransport::new().with(&catalog("songs/1097861769"), &[], "catalog_song.json");

    let song = endpoints::song(
        &transport,
        STOREFRONT,
        &SongId::from("1097861769".to_owned()),
    )
    .await
    .unwrap();

    assert_eq!(song.id.as_str(), "1097861769");
}

#[tokio::test]
async fn song_rejects_an_id_that_changes_the_path() {
    let transport = FixtureTransport::new();

    let error = endpoints::song(&transport, STOREFRONT, &SongId::from("1/albums".to_owned()))
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidInput { .. }), "got {error:?}");
}

#[tokio::test]
async fn album_returns_the_album_with_tracks() {
    let transport = FixtureTransport::new().with(
        &catalog("albums/1097861387"),
        &[("include", "tracks")],
        "catalog_album.json",
    );

    let album = endpoints::album(
        &transport,
        STOREFRONT,
        &AlbumId::from("1097861387".to_owned()),
    )
    .await
    .unwrap();

    let tracks = album.relationships.unwrap().tracks.unwrap();
    assert!(!tracks.data.is_empty());
}

#[tokio::test]
async fn artist_returns_the_artist_with_views() {
    let transport = FixtureTransport::new().with(
        &catalog("artists/657515"),
        &[("views", "top-songs,full-albums")],
        "catalog_artist.json",
    );

    let artist = endpoints::artist(&transport, STOREFRONT, &ArtistId::from("657515".to_owned()))
        .await
        .unwrap();

    let views = artist.views.unwrap();
    assert!(!views.top_songs.unwrap().data.is_empty());
    assert!(!views.full_albums.unwrap().data.is_empty());
}

#[tokio::test]
async fn playlist_returns_the_playlist_with_tracks() {
    let transport = FixtureTransport::new().with(
        &catalog(&format!("playlists/{PLAYLIST_ID}")),
        &[("include", "tracks")],
        "catalog_playlist.json",
    );

    let playlist = endpoints::playlist(
        &transport,
        STOREFRONT,
        &PlaylistId::from(PLAYLIST_ID.to_owned()),
    )
    .await
    .unwrap();

    let tracks = playlist.relationships.unwrap().tracks.unwrap();
    assert!(!tracks.data.is_empty());
}

#[tokio::test]
async fn playlist_tracks_returns_a_page_with_the_next_offset() {
    let transport = FixtureTransport::new().with(
        &catalog(&format!("playlists/{PLAYLIST_ID}/tracks")),
        &PAGE_QUERY,
        "catalog_playlist_tracks.json",
    );

    let page = endpoints::playlist_tracks(
        &transport,
        STOREFRONT,
        &PlaylistId::from(PLAYLIST_ID.to_owned()),
        0,
    )
    .await
    .unwrap();

    assert_eq!(page.items.len(), 3);
    assert!(matches!(page.items[0], Resource::Song(_)));
    assert_eq!(page.next_offset, Some(3));
}

#[tokio::test]
async fn library_playlists_returns_a_page() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/playlists",
        &PAGE_QUERY,
        "library_playlists.json",
    );

    let page = endpoints::library_playlists(&transport, 0).await.unwrap();

    assert_eq!(page.items.len(), 5);
}

#[tokio::test]
async fn library_albums_returns_a_page_with_the_next_offset() {
    let transport =
        FixtureTransport::new().with("/v1/me/library/albums", &PAGE_QUERY, "library_albums.json");

    let page = endpoints::library_albums(&transport, 0).await.unwrap();

    assert_ne!(page.items.len(), 0);
    assert_eq!(page.next_offset, Some(5));
}

#[tokio::test]
async fn library_artists_returns_a_page_with_the_next_offset() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/artists",
        &PAGE_QUERY,
        "library_artists.json",
    );

    let page = endpoints::library_artists(&transport, 0).await.unwrap();

    assert_ne!(page.items.len(), 0);
    assert_eq!(page.next_offset, Some(5));
}

#[tokio::test]
async fn library_songs_sends_the_offset() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/songs",
        &[("limit", "25"), ("offset", "50")],
        "library_songs.json",
    );

    let page = endpoints::library_songs(&transport, 50).await.unwrap();

    assert_ne!(page.items.len(), 0);
}

#[tokio::test]
async fn library_playlist_returns_the_playlist_with_tracks() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/playlists/p.example1",
        &[("include", "tracks")],
        "library_playlist.json",
    );

    let playlist = endpoints::library_playlist(
        &transport,
        &LibraryPlaylistId::from("p.example1".to_owned()),
    )
    .await
    .unwrap();

    let tracks = playlist.relationships.unwrap().tracks.unwrap();
    assert_eq!(tracks.data.len(), 2);
}

#[tokio::test]
async fn library_playlist_returns_empty_response_without_data() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/playlists/p.example1",
        &[("include", "tracks")],
        "empty_data.json",
    );

    let error = endpoints::library_playlist(
        &transport,
        &LibraryPlaylistId::from("p.example1".to_owned()),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, Error::EmptyResponse), "got {error:?}");
}

#[tokio::test]
async fn library_playlist_tracks_returns_a_page_with_the_next_offset() {
    let transport = FixtureTransport::new().with(
        "/v1/me/library/playlists/p.example1/tracks",
        &PAGE_QUERY,
        "library_playlist_tracks.json",
    );

    let page = endpoints::library_playlist_tracks(
        &transport,
        &LibraryPlaylistId::from("p.example1".to_owned()),
        0,
    )
    .await
    .unwrap();

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.next_offset, Some(2));
}

#[tokio::test]
async fn recommendations_returns_the_groups() {
    let transport =
        FixtureTransport::new().with("/v1/me/recommendations", &[], "recommendations.json");

    let groups = endpoints::recommendations(&transport).await.unwrap();

    assert_ne!(groups.len(), 0);
}
