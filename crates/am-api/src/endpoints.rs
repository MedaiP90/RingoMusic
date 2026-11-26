// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Typed requests to the Apple Music API.
//!
//! Each function sends one request through an [`ApiTransport`] and parses the
//! response. Catalog functions take the storefront, for example `it`. Functions
//! that return a [`Page`] take the `offset` of the page. Start with `0`, then
//! use [`Page::next_offset`].

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::models::{
    Album, AlbumId, ApiPage, Artist, ArtistId, Group, LibraryAlbum, LibraryAlbumId, LibraryArtist,
    LibraryArtistId, LibraryPlaylist, LibraryPlaylistId, LibrarySearchResults, LibrarySong,
    LibrarySongId, MapResponse, Playlist, PlaylistId, Recommendation, Resource, SearchResults,
    Song, SongId,
};
use crate::{ApiTransport, Error, PAGE_SIZE, Page, Result};

/// Kind of resource in a catalog search.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum CatalogSearchType {
    /// Songs.
    Songs,
    /// Albums.
    Albums,
    /// Artists.
    Artists,
    /// Playlists.
    Playlists,
}

impl CatalogSearchType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Songs => "songs",
            Self::Albums => "albums",
            Self::Artists => "artists",
            Self::Playlists => "playlists",
        }
    }
}

/// Kind of resource in a library search.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum LibrarySearchType {
    /// Songs.
    Songs,
    /// Albums.
    Albums,
    /// Artists.
    Artists,
    /// Playlists.
    Playlists,
}

impl LibrarySearchType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Songs => "library-songs",
            Self::Albums => "library-albums",
            Self::Artists => "library-artists",
            Self::Playlists => "library-playlists",
        }
    }
}

/// Searches the catalog. The result includes the top results. Each song
/// has its `relationships` with the album and the artist.
///
/// `limit` is the maximum number of results for each type. Apple accepts
/// values from 1 to 25 and rejects larger values. `offset` is the position
/// of the first result of each type. The `next` field of each list gives the
/// offset of the next page.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `term` is empty or `types` is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn search(
    transport: &impl ApiTransport,
    storefront: &str,
    term: &str,
    types: &[CatalogSearchType],
    limit: u32,
    offset: u32,
) -> Result<SearchResults> {
    let term = check_term(term)?;
    let types = join_types(types.iter().map(|kind| kind.as_str()))?;
    let path = format!("/v1/catalog/{}/search", segment("storefront", storefront)?);
    let (limit, offset) = (limit.to_string(), offset.to_string());
    let query = [
        ("term", term),
        ("types", &types),
        ("limit", &limit),
        ("offset", &offset),
        ("with", "topResults"),
        // Song results then link to their albums and artists.
        ("include[songs]", "albums,artists"),
    ];
    let envelope: SearchEnvelope<SearchResults> = fetch(transport, &path, &query).await?;
    Ok(envelope.results)
}

/// Searches the library of the user.
///
/// `limit` is the maximum number of results for each type. Apple accepts
/// values from 1 to 25 and rejects larger values. `offset` is the position
/// of the first result of each type. The `next` field of each list gives the
/// offset of the next page.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `term` is empty or `types` is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn library_search(
    transport: &impl ApiTransport,
    term: &str,
    types: &[LibrarySearchType],
    limit: u32,
    offset: u32,
) -> Result<LibrarySearchResults> {
    let term = check_term(term)?;
    let types = join_types(types.iter().map(|kind| kind.as_str()))?;
    let (limit, offset) = (limit.to_string(), offset.to_string());
    let query = [
        ("term", term),
        ("types", &types),
        ("limit", &limit),
        ("offset", &offset),
        ("include[library-artists]", "catalog"),
        ("fields[artists]", CATALOG_ARTIST_FIELDS),
    ];
    let envelope: SearchEnvelope<LibrarySearchResults> =
        fetch(transport, "/v1/me/library/search", &query).await?;
    Ok(envelope.results)
}

/// Gets one catalog song with its albums and artists.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns [`Error::EmptyResponse`] if the song does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn song(transport: &impl ApiTransport, storefront: &str, id: &SongId) -> Result<Song> {
    let path = catalog_path(storefront, "songs", id.as_str())?;
    first(fetch(transport, &path, &[("include", "albums,artists")]).await?)
}

/// Gets one catalog album with its tracks.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns [`Error::EmptyResponse`] if the album does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn album(transport: &impl ApiTransport, storefront: &str, id: &AlbumId) -> Result<Album> {
    let path = catalog_path(storefront, "albums", id.as_str())?;
    first(fetch(transport, &path, &[("include", "tracks")]).await?)
}

/// Attributes of the catalog artist that a library artist request includes.
/// The library artist has no photo, so the request takes it from the catalog
/// artist.
const CATALOG_ARTIST_FIELDS: &str = "artwork,name";

/// Views of a catalog artist that [`artist`] gets. [`artist_view`] gets the
/// next pages of one view. The app cannot play music videos, so the
/// `top-music-videos` view is not here. The `playlists` view is not here
/// because it repeats `featured-playlists`.
pub const ARTIST_VIEWS: &str = "latest-release,top-songs,full-albums,singles,live-albums,\
compilation-albums,featured-playlists,appears-on-albums,similar-artists";

/// Gets one catalog artist with the first page of each view in
/// [`ARTIST_VIEWS`], and with the biography, the origin, and the start date.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns [`Error::EmptyResponse`] if the artist does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn artist(
    transport: &impl ApiTransport,
    storefront: &str,
    id: &ArtistId,
) -> Result<Artist> {
    let path = catalog_path(storefront, "artists", id.as_str())?;
    let query = [
        ("views", ARTIST_VIEWS),
        ("extend", "artistBio,bornOrFormed,origin,isGroup"),
    ];
    first(fetch(transport, &path, &query).await?)
}

/// Gets one page of the top songs of a catalog artist.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn artist_top_songs(
    transport: &impl ApiTransport,
    storefront: &str,
    id: &ArtistId,
    offset: u32,
) -> Result<Page<Song>> {
    let path = artist_view_path(storefront, id, "top-songs")?;
    fetch_page(transport, &path, offset).await
}

/// Gets one page of the view `view` of a catalog artist, for example
/// `full-albums`. `T` is the resource type of the view: [`Album`] for the
/// album views, [`Playlist`] for `featured-playlists`, and [`Artist`] for
/// `similar-artists`.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn artist_view<T: DeserializeOwned>(
    transport: &impl ApiTransport,
    storefront: &str,
    id: &ArtistId,
    view: &str,
    offset: u32,
) -> Result<Page<T>> {
    let path = artist_view_path(storefront, id, view)?;
    fetch_page(transport, &path, offset).await
}

/// Gets one catalog playlist with its first tracks. Use [`playlist_tracks`]
/// for the next tracks.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns [`Error::EmptyResponse`] if the playlist does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn playlist(
    transport: &impl ApiTransport,
    storefront: &str,
    id: &PlaylistId,
) -> Result<Playlist> {
    let path = catalog_path(storefront, "playlists", id.as_str())?;
    first(fetch(transport, &path, &[("include", "tracks")]).await?)
}

/// Gets one page of the tracks of a catalog playlist.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if an argument is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn playlist_tracks(
    transport: &impl ApiTransport,
    storefront: &str,
    id: &PlaylistId,
    offset: u32,
) -> Result<Page<Resource>> {
    let path = format!(
        "{}/tracks",
        catalog_path(storefront, "playlists", id.as_str())?
    );
    fetch_page(transport, &path, offset).await
}

/// Gets one page of the playlists in the library.
///
/// # Errors
///
/// Returns an error if the request fails or the response is not valid.
pub async fn library_playlists(
    transport: &impl ApiTransport,
    offset: u32,
) -> Result<Page<LibraryPlaylist>> {
    fetch_page(transport, "/v1/me/library/playlists", offset).await
}

/// Gets one page of the albums in the library.
///
/// # Errors
///
/// Returns an error if the request fails or the response is not valid.
pub async fn library_albums(
    transport: &impl ApiTransport,
    offset: u32,
) -> Result<Page<LibraryAlbum>> {
    fetch_page(transport, "/v1/me/library/albums", offset).await
}

/// Gets one page of the artists in the library.
///
/// # Errors
///
/// Returns an error if the request fails or the response is not valid.
pub async fn library_artists(
    transport: &impl ApiTransport,
    offset: u32,
) -> Result<Page<LibraryArtist>> {
    let query = [
        ("include", "catalog"),
        ("fields[artists]", CATALOG_ARTIST_FIELDS),
    ];
    fetch_page_with(transport, "/v1/me/library/artists", offset, &query).await
}

/// Gets one page of the songs in the library.
///
/// # Errors
///
/// Returns an error if the request fails or the response is not valid.
pub async fn library_songs(
    transport: &impl ApiTransport,
    offset: u32,
) -> Result<Page<LibrarySong>> {
    fetch_page(transport, "/v1/me/library/songs", offset).await
}

/// Gets one library song with its albums and artists.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the ID is empty.
/// Returns [`Error::EmptyResponse`] if the song does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn library_song(
    transport: &impl ApiTransport,
    id: &LibrarySongId,
) -> Result<LibrarySong> {
    let path = library_path("songs", id.as_str())?;
    first(fetch(transport, &path, &[("include", "albums,artists")]).await?)
}

/// Gets one library album with its tracks.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the ID is empty.
/// Returns [`Error::EmptyResponse`] if the album does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn library_album(
    transport: &impl ApiTransport,
    id: &LibraryAlbumId,
) -> Result<LibraryAlbum> {
    let path = library_path("albums", id.as_str())?;
    first(fetch(transport, &path, &[("include", "tracks,artists")]).await?)
}

/// Gets one page of the albums of a library artist.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the ID is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn library_artist_albums(
    transport: &impl ApiTransport,
    id: &LibraryArtistId,
    offset: u32,
) -> Result<Page<LibraryAlbum>> {
    let path = format!("{}/albums", library_path("artists", id.as_str())?);
    fetch_page(transport, &path, offset).await
}

/// Gets one library playlist with its first tracks. Use
/// [`library_playlist_tracks`] for the next tracks.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the ID is empty.
/// Returns [`Error::EmptyResponse`] if the playlist does not exist.
/// Returns an error if the request fails or the response is not valid.
pub async fn library_playlist(
    transport: &impl ApiTransport,
    id: &LibraryPlaylistId,
) -> Result<LibraryPlaylist> {
    let path = library_playlist_path(id)?;
    first(fetch(transport, &path, &[("include", "tracks")]).await?)
}

/// Gets one page of the tracks of a library playlist.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the ID is empty.
/// Returns an error if the request fails or the response is not valid.
pub async fn library_playlist_tracks(
    transport: &impl ApiTransport,
    id: &LibraryPlaylistId,
    offset: u32,
) -> Result<Page<Resource>> {
    let path = format!("{}/tracks", library_playlist_path(id)?);
    fetch_page(transport, &path, offset).await
}

/// Gets the recommendations of the user.
///
/// # Errors
///
/// Returns an error if the request fails or the response is not valid.
pub async fn recommendations(transport: &impl ApiTransport) -> Result<Vec<Recommendation>> {
    let page: ApiPage<Recommendation> = fetch(transport, "/v1/me/recommendations", &[]).await?;
    Ok(page.data.into_vec())
}

/// Gets the groups of the New page. Groups of unsupported types are left out.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `storefront` is not a valid path segment.
/// Returns an error if the request fails or the response is not valid.
pub async fn new_releases(transport: &impl ApiTransport, storefront: &str) -> Result<Vec<Group>> {
    let path = format!(
        "/v1/editorial/{}/groupings",
        segment("storefront", storefront)?
    );
    let query = [
        ("name", "music"),
        ("tabs", "subscriber"),
        ("platform", "web"),
        ("format[resources]", "map"),
        ("art[url]", "c,f"),
        ("extend", "artistUrl,editorialArtwork,plainEditorialNotes"),
        (
            "fields[albums]",
            "artistName,artistUrl,artwork,contentRating,editorialArtwork,plainEditorialNotes,name,playParams,releaseDate,url,trackCount",
        ),
        ("include[albums]", "artists"),
        ("include[songs]", "artists"),
        ("omit[resource:artists]", "autos"),
        ("relate[songs]", "albums"),
    ];
    let response: MapResponse = fetch(transport, &path, &query).await?;
    Ok(response.new_page_groups())
}

/// Gets the "Made For You" playlists of the user as one group.
///
/// # Errors
///
/// Returns an error if the request fails or the response is not valid.
pub async fn made_for_you(transport: &impl ApiTransport) -> Result<Vec<Group>> {
    let query = [
        ("filter[featured]", "made-for-you"),
        ("format[resources]", "map"),
        ("include[library-playlists]", "catalog"),
        ("art[url]", "f"),
        (
            "fields[library-playlists]",
            "artwork,dateAdded,name,playParams",
        ),
        ("fields[playlists]", "lastModifiedDate"),
        ("sort", "type"),
        ("platform", "web"),
    ];
    let response: MapResponse = fetch(transport, "/v1/me/library/playlists", &query).await?;
    Ok(response.flat_groups())
}

/// Search responses wrap the results in a `results` object.
#[derive(serde::Deserialize)]
struct SearchEnvelope<T> {
    results: T,
}

/// Sends a request and parses the response into `T`.
async fn fetch<T: DeserializeOwned>(
    transport: &impl ApiTransport,
    path: &str,
    query: &[(&str, &str)],
) -> Result<T> {
    let value: Value = transport.get(path, query).await?;
    serde_json::from_value(value).map_err(Error::Parse)
}

/// Sends a list request for the page at `offset`.
async fn fetch_page<T: DeserializeOwned>(
    transport: &impl ApiTransport,
    path: &str,
    offset: u32,
) -> Result<Page<T>> {
    fetch_page_with(transport, path, offset, &[]).await
}

/// Sends a list request for the page at `offset`, with the parameters in
/// `extra`.
async fn fetch_page_with<T: DeserializeOwned>(
    transport: &impl ApiTransport,
    path: &str,
    offset: u32,
    extra: &[(&str, &str)],
) -> Result<Page<T>> {
    let limit = PAGE_SIZE.to_string();
    let offset = offset.to_string();
    let mut query = vec![("limit", limit.as_str()), ("offset", offset.as_str())];
    query.extend_from_slice(extra);
    let page: ApiPage<T> = fetch(transport, path, &query).await?;
    Ok(page.into())
}

/// Takes the one resource of a detail response.
pub(crate) fn first<T>(page: ApiPage<T>) -> Result<T> {
    page.data.into_iter().next().ok_or(Error::EmptyResponse)
}

pub(crate) fn catalog_path(storefront: &str, kind: &str, id: &str) -> Result<String> {
    let storefront = segment("storefront", storefront)?;
    let id = segment("ID", id)?;
    Ok(format!("/v1/catalog/{storefront}/{kind}/{id}"))
}

fn artist_view_path(storefront: &str, id: &ArtistId, view: &str) -> Result<String> {
    let artist = catalog_path(storefront, "artists", id.as_str())?;
    Ok(format!("{artist}/view/{view}"))
}

fn library_playlist_path(id: &LibraryPlaylistId) -> Result<String> {
    library_path("playlists", id.as_str())
}

fn library_path(kind: &str, id: &str) -> Result<String> {
    let id = segment("ID", id)?;
    Ok(format!("/v1/me/library/{kind}/{id}"))
}

/// Checks a value that goes into the path. It must not be empty or change the
/// structure of the path.
pub(crate) fn segment<'a>(name: &str, value: &'a str) -> Result<&'a str> {
    if value.is_empty() || value.contains(['/', '?', '#']) {
        return Err(invalid(format!(
            "the {name} is empty or has a bad character"
        )));
    }
    Ok(value)
}

/// Checks the search term. Returns it without the surrounding spaces.
fn check_term(term: &str) -> Result<&str> {
    let term = term.trim();
    if term.is_empty() {
        return Err(invalid("the search term is empty".to_owned()));
    }
    Ok(term)
}

/// Joins the search types for the `types` parameter.
fn join_types<'a>(types: impl Iterator<Item = &'a str>) -> Result<String> {
    let types: Vec<&str> = types.collect();
    if types.is_empty() {
        return Err(invalid("the list of search types is empty".to_owned()));
    }
    Ok(types.join(","))
}

pub(crate) fn invalid(message: String) -> Error {
    Error::InvalidInput { message }
}
