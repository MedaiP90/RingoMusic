// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Data of the album, artist, and playlist pages, and the requests that load
//! it. An ID tells if the item is in the library or in the catalog.

use std::time::Duration;

use gettextrs::gettext;
use ringo_am_api::endpoints;
use ringo_am_api::models::{
    Album, AlbumId, ApiPage, Artist, ArtistId, ArtistViews, LibraryAlbum, LibraryAlbumId,
    LibraryArtistId, LibraryPlaylist, LibraryPlaylistId, LibrarySongId, Playlist, PlaylistId,
    Relationship, Resource, SongId,
};
use ringo_am_api::{Page, Result as ApiResult};

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
    /// ID of the catalog album, for the favorite star. A library album has one only if Apple
    /// links it.
    pub catalog_id: Option<String>,
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
            catalog_id: Some(album.id.as_str().to_owned()),
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
            catalog_id: album
                .relationships
                .as_ref()
                .and_then(|links| first_id(links.catalog.as_ref())),
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

/// When a catalog artist started: the birth year of a person or the start
/// year of a group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtistStart {
    Born(String),
    Formed(String),
}

/// A list of tiles on an artist page. Each list is one view of the API.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArtistList {
    LatestRelease,
    Albums,
    Singles,
    LiveAlbums,
    Compilations,
    FeaturedPlaylists,
    AppearsOn,
    SimilarArtists,
}

impl ArtistList {
    /// All lists, in the order of the artist page.
    pub const ALL: [Self; 8] = [
        Self::LatestRelease,
        Self::Albums,
        Self::Singles,
        Self::LiveAlbums,
        Self::Compilations,
        Self::FeaturedPlaylists,
        Self::AppearsOn,
        Self::SimilarArtists,
    ];

    /// Name of the API view of the list.
    pub fn view(self) -> &'static str {
        match self {
            Self::LatestRelease => "latest-release",
            Self::Albums => "full-albums",
            Self::Singles => "singles",
            Self::LiveAlbums => "live-albums",
            Self::Compilations => "compilation-albums",
            Self::FeaturedPlaylists => "featured-playlists",
            Self::AppearsOn => "appears-on-albums",
            Self::SimilarArtists => "similar-artists",
        }
    }

    /// Heading of the list.
    pub fn title(self) -> String {
        match self {
            Self::LatestRelease => gettext("Latest Release"),
            Self::Albums => gettext("Albums"),
            Self::Singles => gettext("Singles & EPs"),
            Self::LiveAlbums => gettext("Live Albums"),
            Self::Compilations => gettext("Compilations"),
            Self::FeaturedPlaylists => gettext("Playlists"),
            Self::AppearsOn => gettext("Appears On"),
            Self::SimilarArtists => gettext("Similar Artists"),
        }
    }

    /// The first page of the list in `views`.
    fn first_page(self, views: &ArtistViews) -> Page<TileItem> {
        match self {
            Self::LatestRelease => tile_page(views.latest_release.as_ref()),
            Self::Albums => tile_page(views.full_albums.as_ref()),
            Self::Singles => tile_page(views.singles.as_ref()),
            Self::LiveAlbums => tile_page(views.live_albums.as_ref()),
            Self::Compilations => tile_page(views.compilation_albums.as_ref()),
            Self::FeaturedPlaylists => tile_page(views.featured_playlists.as_ref()),
            Self::AppearsOn => tile_page(views.appears_on_albums.as_ref()),
            Self::SimilarArtists => tile_page(views.similar_artists.as_ref()),
        }
    }
}

/// The tiles of one list of an artist page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistTiles {
    pub list: ArtistList,
    pub items: Vec<TileItem>,
    /// Offset of the next page, if the list has more.
    pub next_offset: Option<u32>,
}

impl ArtistTiles {
    fn new(list: ArtistList, page: Page<TileItem>) -> Self {
        Self {
            list,
            items: page.items,
            next_offset: page.next_offset,
        }
    }
}

/// The data of an artist page. The genres, the origin, the start, and the
/// biography are empty for a library artist: the API has no data for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistData {
    pub name: String,
    pub artwork_template: Option<String>,
    pub genres: Vec<String>,
    pub origin: Option<String>,
    pub start: Option<ArtistStart>,
    /// Plain text.
    pub biography: Option<String>,
    /// Empty for a library artist: the API has no top songs for it.
    pub top_songs: Vec<SongItem>,
    /// Offset of the next page of top songs, if the artist has more.
    pub top_songs_next: Option<u32>,
    /// The lists of tiles in the order of the page. A library artist has
    /// only the albums: the API has no other lists for it.
    pub lists: Vec<ArtistTiles>,
}

impl ArtistData {
    /// The tiles of `list`. `None` if the artist has no such list.
    pub fn tiles(&self, list: ArtistList) -> Option<&ArtistTiles> {
        self.lists.iter().find(|tiles| tiles.list == list)
    }
}

impl ArtistData {
    fn from_catalog(artist: &Artist) -> Self {
        let attributes = &artist.attributes;
        let views = artist.views.as_deref();
        let songs = views
            .and_then(|views| views.top_songs.clone())
            .map_or_else(empty_page, Page::from);
        let lists = views
            .map(|views| {
                ArtistList::ALL
                    .into_iter()
                    .map(|list| ArtistTiles::new(list, list.first_page(views)))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            name: attributes.name.clone(),
            artwork_template: attributes.artwork.as_ref().map(|art| art.url.clone()),
            genres: attributes.genre_names.clone(),
            origin: attributes
                .origin
                .as_deref()
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned),
            start: start_of(attributes.born_or_formed.as_deref(), attributes.is_group),
            biography: attributes.artist_bio.as_deref().and_then(plain_text),
            top_songs: songs.items.iter().map(SongItem::from).collect(),
            top_songs_next: songs.next_offset,
            lists,
        }
    }
}

/// The tiles of the API page `page`. An empty page if `page` is `None`.
fn tile_page<T: Clone>(page: Option<&ApiPage<T>>) -> Page<TileItem>
where
    for<'item> TileItem: From<&'item T>,
{
    page.cloned()
        .map_or_else(empty_page, |page| tiles_of(&Page::from(page)))
}

fn tiles_of<T>(page: &Page<T>) -> Page<TileItem>
where
    for<'item> TileItem: From<&'item T>,
{
    Page {
        items: page.items.iter().map(TileItem::from).collect(),
        next_offset: page.next_offset,
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
    /// ID of the catalog playlist, for the favorite star. A library playlist has one only if
    /// Apple links it.
    pub catalog_id: Option<String>,
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
            catalog_id: Some(playlist.id.as_str().to_owned()),
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
            catalog_id: playlist
                .relationships
                .as_ref()
                .and_then(|links| first_id(links.catalog.as_ref())),
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

/// Loads the albums of the library artist `id`, up to
/// [`ARTIST_ALBUM_PAGES_LIMIT`] pages.
///
/// # Errors
///
/// Returns an error if a request fails.
pub async fn load_library_artist_albums(api: &ApiClient, id: &str) -> ApiResult<Vec<TileItem>> {
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
    Ok(albums)
}

/// A library artist has a name and albums. The name comes from the first
/// album, because the API has no request for one library artist.
async fn load_library_artist(api: &ApiClient, id: &str) -> ApiResult<ArtistData> {
    let albums = load_library_artist_albums(api, id).await?;
    let name = albums
        .first()
        .map(|album| album.subtitle.clone())
        .unwrap_or_default();
    Ok(ArtistData {
        name,
        artwork_template: None,
        genres: Vec::new(),
        origin: None,
        start: None,
        biography: None,
        top_songs: Vec::new(),
        top_songs_next: None,
        lists: vec![ArtistTiles {
            list: ArtistList::Albums,
            items: albums,
            next_offset: None,
        }],
    })
}

/// Loads the album and the artist of a song, for the links of the player.
/// `id` is a catalog ID or a library ID.
///
/// # Errors
///
/// Returns an error if the request fails or the song does not exist.
pub async fn load_song_links(api: &ApiClient, storefront: &str, id: &str) -> ApiResult<SongLinks> {
    let (links, catalog_id) = if is_library_id(id) {
        let song = endpoints::library_song(api, &LibrarySongId::from(id.to_owned())).await?;
        let catalog_id = song
            .attributes
            .play_params
            .and_then(|params| params.catalog_id);
        let links = song
            .relationships
            .map(|links| (links.albums, links.artists));
        (links, catalog_id)
    } else {
        let song = endpoints::song(api, storefront, &SongId::from(id.to_owned())).await?;
        let links = song
            .relationships
            .map(|links| (links.albums, links.artists));
        (links, Some(id.to_owned()))
    };
    let (albums, artists) = links.unwrap_or_default();
    Ok(SongLinks {
        album_id: first_id(albums.as_ref()),
        artist_id: first_id(artists.as_ref()),
        catalog_id,
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

/// Loads the page of `list` of a catalog artist at `offset`.
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn load_artist_tiles(
    api: &ApiClient,
    storefront: &str,
    id: &str,
    list: ArtistList,
    offset: u32,
) -> ApiResult<Page<TileItem>> {
    let id = ArtistId::from(id.to_owned());
    let view = list.view();
    let page = match list {
        ArtistList::SimilarArtists => {
            tiles_of(&endpoints::artist_view::<Artist>(api, storefront, &id, view, offset).await?)
        }
        ArtistList::FeaturedPlaylists => {
            tiles_of(&endpoints::artist_view::<Playlist>(api, storefront, &id, view, offset).await?)
        }
        ArtistList::LatestRelease
        | ArtistList::Albums
        | ArtistList::Singles
        | ArtistList::LiveAlbums
        | ArtistList::Compilations
        | ArtistList::AppearsOn => {
            tiles_of(&endpoints::artist_view::<Album>(api, storefront, &id, view, offset).await?)
        }
    };
    Ok(page)
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

/// Reads the start year of an artist. The API marks a person with
/// `is_group: false`. Without the mark, the artist counts as a group.
fn start_of(born_or_formed: Option<&str>, is_group: Option<bool>) -> Option<ArtistStart> {
    let year = year_of(born_or_formed)?;
    Some(if is_group == Some(false) {
        ArtistStart::Born(year)
    } else {
        ArtistStart::Formed(year)
    })
}

/// Removes the HTML tags from `html` and trims it. `None` for a blank text.
// ponytail: drops tags only, the API sends only `<i>` and Unicode characters.
// Decode entities if a biography shows `&amp;` or similar.
fn plain_text(html: &str) -> Option<String> {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
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
    fn artist_from_catalog_reads_the_lists_in_page_order() {
        let artist: Artist = serde_json::from_value(json!({
            "id": "1",
            "type": "artists",
            "attributes": { "name": "Band" },
            "views": {
                "similar-artists": {
                    "data": [{ "id": "2", "type": "artists", "attributes": { "name": "Other" } }]
                },
                "singles": {
                    "data": [{ "id": "3", "type": "albums", "attributes": { "name": "Single" } }],
                    "next": "/v1/catalog/it/artists/1/view/singles?offset=1"
                }
            }
        }))
        .expect("the test artist is valid");

        let data = ArtistData::from_catalog(&artist);

        let lists: Vec<ArtistList> = data.lists.iter().map(|tiles| tiles.list).collect();
        assert_eq!(lists, ArtistList::ALL);
        let similar = data
            .tiles(ArtistList::SimilarArtists)
            .expect("the similar artists are listed");
        assert_eq!(similar.items[0].title, "Other");
        let singles = data
            .tiles(ArtistList::Singles)
            .expect("the singles are listed");
        assert_eq!(
            (singles.items[0].title.as_str(), singles.next_offset),
            ("Single", Some(1))
        );
        assert_eq!(
            data.tiles(ArtistList::Albums)
                .expect("the albums are listed")
                .items,
            Vec::new()
        );
    }

    #[test]
    fn start_of_tells_a_person_from_a_group() {
        assert_eq!(
            start_of(Some("1965-05-17"), Some(false)),
            Some(ArtistStart::Born("1965".to_owned()))
        );
        assert_eq!(
            start_of(Some("1985"), Some(true)),
            Some(ArtistStart::Formed("1985".to_owned()))
        );
        assert_eq!(start_of(None, Some(true)), None);
    }

    #[test]
    fn plain_text_removes_the_tags() {
        assert_eq!(
            plain_text(" The album <i>OK Computer</i> (1997) ").as_deref(),
            Some("The album OK Computer (1997)")
        );
        assert_eq!(plain_text("<br/> "), None);
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
}
