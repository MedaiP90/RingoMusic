// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tests that the recorded API responses deserialize into the models.

#![expect(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::path::PathBuf;

use ringo_am_api::models::{
    Album, ApiPage, Artist, LibraryAlbum, LibraryArtist, LibraryPlaylist, LibrarySearchResults,
    LibrarySong, Playlist, Recommendation, Resource, SearchResults, Song,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Deserialize)]
struct Search {
    results: SearchResults,
}

#[derive(Deserialize)]
struct LibrarySearch {
    results: LibrarySearchResults,
}

fn fixture_text(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/api")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// Parses a `{ "data": [...] }` fixture. Checks that no element was skipped.
fn page<T: DeserializeOwned>(name: &str) -> ApiPage<T> {
    let text = fixture_text(name);
    let raw: Value = serde_json::from_str(&text).unwrap();
    let parsed: ApiPage<T> = serde_json::from_str(&text).unwrap();
    assert_eq!(
        parsed.data.len(),
        raw["data"].as_array().unwrap().len(),
        "{name}: skipped item"
    );
    parsed
}

#[test]
fn catalog_song_deserializes() {
    let page = page::<Song>("catalog_song.json");

    let song = &page.data[0];
    assert!(song.attributes.duration_millis.is_some());
    assert!(song.attributes.play_params.is_some());
    assert!(song.attributes.artwork.is_some());
}

#[test]
fn catalog_album_deserializes_with_tracks() {
    let page = page::<Album>("catalog_album.json");

    let tracks = page.data[0]
        .relationships
        .as_ref()
        .unwrap()
        .tracks
        .as_ref()
        .unwrap();
    assert!(!tracks.data.is_empty());
    assert!(
        tracks
            .data
            .iter()
            .all(|track| matches!(track, Resource::Song(_)))
    );
}

#[test]
fn catalog_artist_deserializes_with_views() {
    let page = page::<Artist>("catalog_artist.json");

    let views = page.data[0].views.as_ref().unwrap();
    assert!(!views.top_songs.as_ref().unwrap().data.is_empty());
    assert!(!views.full_albums.as_ref().unwrap().data.is_empty());
    assert!(
        page.data[0]
            .relationships
            .as_ref()
            .unwrap()
            .albums
            .is_some()
    );
}

#[test]
fn catalog_playlist_deserializes_with_tracks() {
    let page = page::<Playlist>("catalog_playlist.json");

    let tracks = page.data[0]
        .relationships
        .as_ref()
        .unwrap()
        .tracks
        .as_ref()
        .unwrap();
    assert!(!tracks.data.is_empty());
}

#[test]
fn catalog_search_deserializes() {
    let search: Search = serde_json::from_str(&fixture_text("catalog_search.json")).unwrap();

    let results = search.results;
    assert_eq!(results.songs.unwrap().data.len(), 5);
    assert_eq!(results.albums.unwrap().data.len(), 5);
    assert_eq!(results.artists.unwrap().data.len(), 4);
    assert_eq!(results.playlists.unwrap().data.len(), 5);
}

#[test]
fn broken_item_is_skipped_and_the_others_stay() {
    let search: Search =
        serde_json::from_str(&fixture_text("catalog_search_broken_item.json")).unwrap();

    let results = search.results;
    assert_eq!(results.songs.unwrap().data.len(), 4);
    assert_eq!(results.albums.unwrap().data.len(), 5);
    assert_eq!(results.artists.unwrap().data.len(), 4);
    assert_eq!(results.playlists.unwrap().data.len(), 5);
}

#[test]
fn library_songs_deserialize() {
    let page = page::<LibrarySong>("library_songs.json");

    assert!(page.next.is_some());
    assert!(
        page.data[0]
            .attributes
            .play_params
            .as_ref()
            .unwrap()
            .catalog_id
            .is_some()
    );
}

#[test]
fn library_albums_deserialize() {
    let page = page::<LibraryAlbum>("library_albums.json");

    assert!(!page.data.is_empty());
}

#[test]
fn library_artists_deserialize() {
    let page = page::<LibraryArtist>("library_artists.json");

    assert!(!page.data.is_empty());
}

#[test]
fn library_playlists_deserialize() {
    let page = page::<LibraryPlaylist>("library_playlists.json");

    assert!(!page.data.is_empty());
}

#[test]
fn recommendations_deserialize_with_mixed_contents() {
    let page = page::<Recommendation>("recommendations.json");

    assert!(
        page.data
            .iter()
            .all(|recommendation| recommendation.title.is_some())
    );
    let contents: Vec<&Resource> = page
        .data
        .iter()
        .flat_map(|recommendation| recommendation.contents.iter())
        .collect();
    assert!(
        contents
            .iter()
            .any(|resource| matches!(resource, Resource::Album(_)))
    );
    assert!(
        contents
            .iter()
            .any(|resource| matches!(resource, Resource::Playlist(_)))
    );
    assert!(
        contents
            .iter()
            .any(|resource| matches!(resource, Resource::Unknown))
    );
}

#[test]
fn catalog_search_top_results_deserialize_with_mixed_types() {
    let search: Search = serde_json::from_str(&fixture_text("catalog_search_top.json")).unwrap();

    let top = search.results.top_results.unwrap();
    assert_eq!(top.data.len(), 5);
    assert!(
        top.data
            .iter()
            .any(|resource| matches!(resource, Resource::Artist(_)))
    );
    assert!(
        top.data
            .iter()
            .any(|resource| matches!(resource, Resource::Song(_)))
    );
    assert!(
        top.data
            .iter()
            .any(|resource| matches!(resource, Resource::Playlist(_)))
    );
}

#[test]
fn catalog_search_without_top_results_has_none() {
    let search: Search = serde_json::from_str(&fixture_text("catalog_search.json")).unwrap();

    assert!(search.results.top_results.is_none());
}

#[test]
fn library_search_deserializes() {
    let search: LibrarySearch = serde_json::from_str(&fixture_text("library_search.json")).unwrap();

    let results = search.results;
    assert_eq!(results.songs.unwrap().data.len(), 5);
    assert_eq!(results.artists.unwrap().data.len(), 1);
    assert!(results.albums.is_none());
    assert!(results.playlists.is_none());
}
