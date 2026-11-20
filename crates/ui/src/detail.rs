// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Data of the album, artist, and playlist pages, and the requests that load
//! it. An ID tells if the item is in the library or in the catalog.

use std::time::Duration;

use ringo_am_api::endpoints;
use ringo_am_api::models::{
    Album, AlbumId, Artist, ArtistId, LibraryAlbum, LibraryAlbumId, LibraryArtistId,
    LibraryPlaylist, LibraryPlaylistId, LibrarySongId, Playlist, PlaylistId, Relationship,
    Resource, SongId,
};
use ringo_am_api::{Page, Result as ApiResult};
use ringo_core::state::{AppState, AuthState};

use crate::item::{SongItem, TileItem, first_id};
use crate::item_actions::SongLinks;
use crate::services::ApiClient;

/// Number of pages of library albums that the artist page loads. A library
/// artist with more albums is cut. The limit stops a loop that never ends.
const ARTIST_ALBUM_PAGES_LIMIT: usize = 8;

/// Number of the disc for a song that has no disc number.
const DEFAULT_DISC: u32 = 1;

/// Length of the year in a release date.
const YEAR_LENGTH: usize = 4;

/// Tells if the ID belongs to the library of the user. Library IDs start
/// with a letter and a dot: `l.` (album), `r.` (artist), `p.` (playlist),
/// and `i.` (song). Catalog IDs are numbers. A catalog playlist starts with
/// `pl.`.
pub fn is_library_id(id: &str) -> bool {
    matches!(id.as_bytes(), [b'l' | b'r' | b'p' | b'i', b'.', ..])
}

/// The storefront of the signed-in user.
pub fn storefront(state: &AppState) -> Option<String> {
    match &state.auth {
        AuthState::SignedIn { storefront } => Some(storefront.clone()),
        AuthState::Unknown | AuthState::SignedOut => None,
    }
}

/// A song of an album, with its place in the album.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Track {
    pub song: SongItem,
    pub disc: u32,
    pub number: Option<u32>,
}

/// The data of an album page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlbumData {
    pub title: String,
    pub artist: String,
    /// ID of the album artist, if the API sent it.
    pub artist_id: Option<String>,
    /// Year of the release, if the API sent a date.
    pub year: Option<String>,
    pub artwork_template: Option<String>,
    pub tracks: Vec<Track>,
}

impl AlbumData {
    fn from_catalog(album: &Album) -> Self {
        let attributes = &album.attributes;
        let artist_id = album
            .relationships
            .as_ref()
            .and_then(|links| links.artists.as_ref())
            .and_then(|artists| artists.data.first())
            .map(|artist| artist.id.clone());
        let tracks = album
            .relationships
            .as_ref()
            .and_then(|links| links.tracks.as_ref());
        Self {
            title: attributes.name.clone(),
            artist: attributes.artist_name.clone().unwrap_or_default(),
            year: year_of(attributes.release_date.as_deref()),
            artwork_template: attributes.artwork.as_ref().map(|art| art.url.clone()),
            tracks: tracks_of(tracks, album.id.as_str(), artist_id.as_deref()),
            artist_id,
        }
    }

    fn from_library(album: &LibraryAlbum) -> Self {
        let attributes = &album.attributes;
        let tracks = album
            .relationships
            .as_ref()
            .and_then(|links| links.tracks.as_ref());
        let artist_id = album
            .relationships
            .as_ref()
            .and_then(|links| links.artists.as_ref())
            .and_then(|artists| artists.data.first())
            .map(|artist| artist.id.clone());
        Self {
            title: attributes.name.clone(),
            artist: attributes.artist_name.clone().unwrap_or_default(),
            year: year_of(attributes.release_date.as_deref()),
            artwork_template: attributes.artwork.as_ref().map(|art| art.url.clone()),
            tracks: tracks_of(tracks, album.id.as_str(), artist_id.as_deref()),
            artist_id,
        }
    }

    /// The songs of the album, in album order.
    pub fn songs(&self) -> Vec<SongItem> {
        self.tracks.iter().map(|track| track.song.clone()).collect()
    }

    /// The sum of the durations of all songs.
    pub fn total_duration(&self) -> Duration {
        self.tracks
            .iter()
            .map(|track| track.song.queue.duration)
            .sum()
    }
}

/// The data of an artist page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistData {
    pub name: String,
    pub artwork_template: Option<String>,
    /// Empty for a library artist: the API has no top songs for it.
    pub top_songs: Vec<SongItem>,
    /// Offset of the next page of top songs, if the artist has more.
    pub top_songs_next: Option<u32>,
    pub albums: Vec<TileItem>,
    /// Offset of the next page of albums, if the artist has more.
    pub albums_next: Option<u32>,
}

impl ArtistData {
    fn from_catalog(artist: &Artist) -> Self {
        let views = artist.views.as_ref();
        let songs = views
            .and_then(|views| views.top_songs.clone())
            .map_or_else(empty_page, Page::from);
        let albums = views
            .and_then(|views| views.full_albums.clone())
            .map_or_else(empty_page, Page::from);
        Self {
            name: artist.attributes.name.clone(),
            artwork_template: artist
                .attributes
                .artwork
                .as_ref()
                .map(|art| art.url.clone()),
            top_songs: songs.items.iter().map(SongItem::from).collect(),
            top_songs_next: songs.next_offset,
            albums: albums.items.iter().map(TileItem::from).collect(),
            albums_next: albums.next_offset,
        }
    }
}

fn empty_page<T>() -> Page<T> {
    Page {
        items: Vec::new(),
        next_offset: None,
    }
}

/// The data of a playlist page.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistData {
    pub title: String,
    /// The curator of a catalog playlist. Empty for a library playlist.
    pub curator: String,
    /// Plain text.
    pub description: Option<String>,
    pub artwork_template: Option<String>,
    /// The first songs. Later songs come from [`load_playlist_songs`].
    pub first_page: Page<SongItem>,
}

impl PlaylistData {
    fn from_catalog(playlist: &Playlist) -> Self {
        let attributes = &playlist.attributes;
        let tracks = playlist
            .relationships
            .as_ref()
            .and_then(|links| links.tracks.clone());
        Self {
            title: attributes.name.clone(),
            curator: attributes.curator_name.clone().unwrap_or_default(),
            description: description_of(attributes.description.as_ref()),
            artwork_template: attributes.artwork.as_ref().map(|art| art.url.clone()),
            first_page: songs_page(tracks),
        }
    }

    fn from_library(playlist: &LibraryPlaylist) -> Self {
        let attributes = &playlist.attributes;
        let tracks = playlist
            .relationships
            .as_ref()
            .and_then(|links| links.tracks.clone());
        Self {
            title: attributes.name.clone(),
            curator: String::new(),
            description: description_of(attributes.description.as_ref()),
            artwork_template: attributes.artwork.as_ref().map(|art| art.url.clone()),
            first_page: songs_page(tracks),
        }
    }
}

/// Loads an album with its songs.
///
/// # Errors
///
/// Returns an error if the request fails or the album does not exist.
pub async fn load_album(api: &ApiClient, storefront: &str, id: &str) -> ApiResult<AlbumData> {
    if is_library_id(id) {
        let id = LibraryAlbumId::from(id.to_owned());
        let album = endpoints::library_album(api, &id).await?;
        return Ok(AlbumData::from_library(&album));
    }
    let id = AlbumId::from(id.to_owned());
    let album = endpoints::album(api, storefront, &id).await?;
    Ok(AlbumData::from_catalog(&album))
}

/// Loads an artist with the top songs and the albums.
///
/// # Errors
///
/// Returns an error if the request fails or the artist does not exist.
pub async fn load_artist(api: &ApiClient, storefront: &str, id: &str) -> ApiResult<ArtistData> {
    if is_library_id(id) {
        return load_library_artist(api, id).await;
    }
    let id = ArtistId::from(id.to_owned());
    let artist = endpoints::artist(api, storefront, &id).await?;
    Ok(ArtistData::from_catalog(&artist))
}

/// A library artist has a name and albums. The name comes from the first
/// album, because the API has no request for one library artist.
async fn load_library_artist(api: &ApiClient, id: &str) -> ApiResult<ArtistData> {
    let id = LibraryArtistId::from(id.to_owned());
    let mut albums = Vec::new();
    let mut offset = Some(0);
    for _ in 0..ARTIST_ALBUM_PAGES_LIMIT {
        let Some(current) = offset else {
            break;
        };
        let page = endpoints::library_artist_albums(api, &id, current).await?;
        albums.extend(page.items.iter().map(TileItem::from));
        offset = page.next_offset;
    }
    let name = albums
        .first()
        .map(|album| album.subtitle.clone())
        .unwrap_or_default();
    Ok(ArtistData {
        name,
        artwork_template: None,
        top_songs: Vec::new(),
        top_songs_next: None,
        albums,
        albums_next: None,
    })
}

/// Loads the album and the artist of a song, for the links of the player.
/// `id` is a catalog ID or a library ID.
///
/// # Errors
///
/// Returns an error if the request fails or the song does not exist.
pub async fn load_song_links(api: &ApiClient, storefront: &str, id: &str) -> ApiResult<SongLinks> {
    let (albums, artists) = if is_library_id(id) {
        let song = endpoints::library_song(api, &LibrarySongId::from(id.to_owned())).await?;
        song.relationships
            .map(|links| (links.albums, links.artists))
    } else {
        let song = endpoints::song(api, storefront, &SongId::from(id.to_owned())).await?;
        song.relationships
            .map(|links| (links.albums, links.artists))
    }
    .unwrap_or_default();
    Ok(SongLinks {
        album_id: first_id(albums.as_ref()),
        artist_id: first_id(artists.as_ref()),
    })
}

/// Loads the page of top songs of a catalog artist at `offset`.
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn load_artist_top_songs(
    api: &ApiClient,
    storefront: &str,
    id: &str,
    offset: u32,
) -> ApiResult<Page<SongItem>> {
    let id = ArtistId::from(id.to_owned());
    let page = endpoints::artist_top_songs(api, storefront, &id, offset).await?;
    Ok(Page {
        items: page.items.iter().map(SongItem::from).collect(),
        next_offset: page.next_offset,
    })
}

/// Loads the page of albums of a catalog artist at `offset`.
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn load_artist_albums(
    api: &ApiClient,
    storefront: &str,
    id: &str,
    offset: u32,
) -> ApiResult<Page<TileItem>> {
    let id = ArtistId::from(id.to_owned());
    let page = endpoints::artist_full_albums(api, storefront, &id, offset).await?;
    Ok(Page {
        items: page.items.iter().map(TileItem::from).collect(),
        next_offset: page.next_offset,
    })
}

/// Loads a playlist with its first songs.
///
/// # Errors
///
/// Returns an error if the request fails or the playlist does not exist.
pub async fn load_playlist(api: &ApiClient, storefront: &str, id: &str) -> ApiResult<PlaylistData> {
    if is_library_id(id) {
        let id = LibraryPlaylistId::from(id.to_owned());
        let playlist = endpoints::library_playlist(api, &id).await?;
        return Ok(PlaylistData::from_library(&playlist));
    }
    let id = PlaylistId::from(id.to_owned());
    let playlist = endpoints::playlist(api, storefront, &id).await?;
    Ok(PlaylistData::from_catalog(&playlist))
}

/// Loads the page of playlist songs at `offset`.
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn load_playlist_songs(
    api: &ApiClient,
    storefront: &str,
    id: &str,
    offset: u32,
) -> ApiResult<Page<SongItem>> {
    let page = if is_library_id(id) {
        let id = LibraryPlaylistId::from(id.to_owned());
        endpoints::library_playlist_tracks(api, &id, offset).await?
    } else {
        let id = PlaylistId::from(id.to_owned());
        endpoints::playlist_tracks(api, storefront, &id, offset).await?
    };
    Ok(Page {
        items: songs_of(&page.items),
        next_offset: page.next_offset,
    })
}

/// The songs of a list of resources. Other resources, for example videos, are
/// skipped.
fn songs_of(resources: &[Resource]) -> Vec<SongItem> {
    resources
        .iter()
        .filter_map(|resource| match resource {
            Resource::Song(song) => Some(SongItem::from(song)),
            Resource::LibrarySong(song) => Some(SongItem::from(song)),
            _ => None,
        })
        .collect()
}

fn songs_page(tracks: Option<Relationship<Resource>>) -> Page<SongItem> {
    let page = tracks.map_or(
        Page {
            items: Vec::new(),
            next_offset: None,
        },
        Page::from,
    );
    Page {
        items: songs_of(&page.items),
        next_offset: page.next_offset,
    }
}

/// The tracks of an album. Each song links to the album, so that its menu can
/// go to it.
fn tracks_of(
    tracks: Option<&Relationship<Resource>>,
    album_id: &str,
    artist_id: Option<&str>,
) -> Vec<Track> {
    let Some(tracks) = tracks else {
        return Vec::new();
    };
    tracks
        .data
        .iter()
        .filter_map(|resource| track_of(resource, album_id, artist_id))
        .collect()
}

fn track_of(resource: &Resource, album_id: &str, artist_id: Option<&str>) -> Option<Track> {
    let (mut song, disc, number) = match resource {
        Resource::Song(song) => (
            SongItem::from(song),
            song.attributes.disc_number,
            song.attributes.track_number,
        ),
        Resource::LibrarySong(song) => (
            SongItem::from(song),
            song.attributes.disc_number,
            song.attributes.track_number,
        ),
        _ => return None,
    };
    song.album_id = Some(album_id.to_owned());
    if song.artist_id.is_none() {
        song.artist_id = artist_id.map(str::to_owned);
    }
    Some(Track {
        song,
        disc: disc.unwrap_or(DEFAULT_DISC),
        number,
    })
}

/// Groups the tracks by disc. The API sends them in disc order.
pub fn discs(tracks: &[Track]) -> Vec<&[Track]> {
    tracks
        .chunk_by(|first, second| first.disc == second.disc)
        .collect()
}

fn year_of(release_date: Option<&str>) -> Option<String> {
    let year = release_date?.get(..YEAR_LENGTH)?;
    year.chars()
        .all(|digit| digit.is_ascii_digit())
        .then(|| year.to_owned())
}

fn description_of(description: Option<&ringo_am_api::models::Description>) -> Option<String> {
    description?
        .standard
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn song_json(id: &str, disc: u32, number: u32) -> serde_json::Value {
        json!({
            "id": id,
            "type": "songs",
            "attributes": {
                "name": id,
                "durationInMillis": 60_000,
                "discNumber": disc,
                "trackNumber": number
            }
        })
    }

    fn album(songs: &[serde_json::Value]) -> Album {
        serde_json::from_value(json!({
            "id": "900",
            "type": "albums",
            "attributes": {
                "name": "Record",
                "artistName": "Band",
                "releaseDate": "1997-05-21"
            },
            "relationships": {
                "tracks": { "data": songs },
                "artists": { "data": [{ "id": "5", "type": "artists" }] }
            }
        }))
        .expect("the test album is valid")
    }

    #[test]
    fn is_library_id_is_true_for_the_four_library_prefixes() {
        for id in ["l.abc", "r.abc", "p.abc", "i.abc"] {
            assert!(is_library_id(id), "{id}");
        }
    }

    #[test]
    fn is_library_id_is_false_for_catalog_ids() {
        for id in ["1440857781", "pl.u-abc", "", "l"] {
            assert!(!is_library_id(id), "{id}");
        }
    }

    #[test]
    fn year_of_reads_the_year_of_a_full_date_and_of_a_year() {
        assert_eq!(year_of(Some("1997-05-21")).as_deref(), Some("1997"));
        assert_eq!(year_of(Some("1997")).as_deref(), Some("1997"));
    }

    #[test]
    fn year_of_returns_none_for_a_missing_or_bad_date() {
        assert_eq!(year_of(None), None);
        assert_eq!(year_of(Some("n/a")), None);
        assert_eq!(year_of(Some("19")), None);
    }

    #[test]
    fn album_from_catalog_keeps_the_track_order_and_adds_the_links() {
        let data = AlbumData::from_catalog(&album(&[song_json("1", 1, 1), song_json("2", 1, 2)]));
        let ids: Vec<&str> = data.tracks.iter().map(|track| track.song.id()).collect();
        assert_eq!(ids, ["1", "2"]);
        assert_eq!(data.tracks[0].song.album_id.as_deref(), Some("900"));
        assert_eq!(data.tracks[0].song.artist_id.as_deref(), Some("5"));
        assert_eq!(data.artist_id.as_deref(), Some("5"));
        assert_eq!(data.year.as_deref(), Some("1997"));
    }

    #[test]
    fn album_total_duration_adds_the_songs() {
        let data = AlbumData::from_catalog(&album(&[song_json("1", 1, 1), song_json("2", 1, 2)]));
        assert_eq!(data.total_duration(), Duration::from_secs(120));
    }

    #[test]
    fn discs_groups_tracks_by_disc_number() {
        let data = AlbumData::from_catalog(&album(&[
            song_json("1", 1, 1),
            song_json("2", 2, 1),
            song_json("3", 2, 2),
        ]));
        let sizes: Vec<usize> = discs(&data.tracks).iter().map(|disc| disc.len()).collect();
        assert_eq!(sizes, [1, 2]);
    }

    #[test]
    fn songs_page_skips_resources_that_are_not_songs() {
        let tracks: Relationship<Resource> = serde_json::from_value(json!({
            "data": [song_json("1", 1, 1), { "id": "9", "type": "music-videos" }],
            "next": "/v1/catalog/it/playlists/pl.x/tracks?offset=100"
        }))
        .expect("the test relationship is valid");
        let page = songs_page(Some(tracks));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.next_offset, Some(100));
    }

    #[test]
    fn songs_page_without_tracks_is_empty() {
        let page = songs_page(None);
        assert_eq!((page.items.len(), page.next_offset), (0, None));
    }

    #[test]
    fn description_of_drops_a_blank_text() {
        let blank = ringo_am_api::models::Description {
            standard: Some("  ".to_owned()),
        };
        assert_eq!(description_of(Some(&blank)), None);
        assert_eq!(description_of(None), None);
    }

    #[test]
    fn storefront_is_none_when_signed_out() {
        let mut state = AppState::default();
        state.auth = AuthState::SignedOut;
        assert_eq!(storefront(&state), None);
    }

    #[test]
    fn storefront_is_the_one_of_the_signed_in_user() {
        let mut state = AppState::default();
        state.auth = AuthState::SignedIn {
            storefront: "it".to_owned(),
        };
        assert_eq!(storefront(&state).as_deref(), Some("it"));
    }
}
