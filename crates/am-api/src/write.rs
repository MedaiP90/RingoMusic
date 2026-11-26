// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Only endpoints that Apple documents in the Apple Music API. Each function needs a link to its
//! documentation page.
//!
//! A write succeeds on any `2xx` status (decision L20). The transport turns each other status into
//! an error, so a function that returns `Ok` has succeeded. `202` means "accepted", not "done".

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::endpoints::{catalog_path, first, invalid, segment};
use crate::models::{
    AlbumId, ApiPage, ArtistId, LibraryPlaylist, LibraryPlaylistId, LibrarySongId, PlaylistId,
    Relationship, ResourceRef, SongId,
};
use crate::{ApiTransport, Error, Result, TransportError, WriteMethod, WriteRequest};

/// Catalog item that a write function accepts. The variant fixes the kind, so the kind and the
/// ID cannot be mixed.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CatalogItem {
    /// A catalog song.
    Song(SongId),
    /// A catalog album.
    Album(AlbumId),
    /// A catalog artist.
    Artist(ArtistId),
    /// A catalog playlist.
    Playlist(PlaylistId),
}

impl CatalogItem {
    /// Returns the type name in API paths and query keys.
    fn kind(&self) -> &'static str {
        match self {
            Self::Song(_) => "songs",
            Self::Album(_) => "albums",
            Self::Artist(_) => "artists",
            Self::Playlist(_) => "playlists",
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Song(id) => id.as_str(),
            Self::Album(id) => id.as_str(),
            Self::Artist(id) => id.as_str(),
            Self::Playlist(id) => id.as_str(),
        }
    }

    /// Fails for an artist. Apple documents the endpoint of `action` only for the other kinds.
    fn reject_artist(&self, action: &str) -> Result<()> {
        match self {
            Self::Artist(_) => Err(invalid(format!("{action} does not support artists"))),
            Self::Song(_) | Self::Album(_) | Self::Playlist(_) => Ok(()),
        }
    }
}

/// Song that a playlist can hold. Apple takes both ID kinds for a track.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Track {
    /// A catalog song.
    Song(SongId),
    /// A song in the library of the user.
    LibrarySong(LibrarySongId),
}

impl Track {
    fn to_json(&self) -> Value {
        match self {
            Self::Song(id) => json!({ "id": id.as_str(), "type": "songs" }),
            Self::LibrarySong(id) => json!({ "id": id.as_str(), "type": "library-songs" }),
        }
    }
}

/// Rating that Apple supports. Apple has no other values.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Rating {
    /// Love. For Apple, this is the same state as a favorite (decision L7).
    Love,
    /// Dislike. It clears the favorite.
    Dislike,
}

impl Rating {
    fn value(self) -> i8 {
        match self {
            Self::Love => 1,
            Self::Dislike => -1,
        }
    }
}

/// Creates a playlist in the library. `tracks` can be empty. Apple adds the tracks in the given
/// order.
///
/// Documentation: <https://developer.apple.com/documentation/applemusicapi/create-a-new-library-playlist>
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `name` is empty. Returns an error if the request fails or
/// the response is not valid.
pub async fn create_playlist(
    transport: &impl ApiTransport,
    name: &str,
    description: Option<&str>,
    tracks: &[Track],
) -> Result<LibraryPlaylist> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("the playlist name is empty".to_owned()));
    }
    let mut attributes = json!({ "name": name });
    if let Some(description) = description.filter(|text| !text.trim().is_empty()) {
        attributes["description"] = json!(description);
    }
    let mut body = json!({ "attributes": attributes });
    if !tracks.is_empty() {
        body["relationships"] = json!({ "tracks": { "data": tracks_json(tracks) } });
    }
    let response = send(
        transport,
        WriteMethod::Post,
        "/v1/me/library/playlists",
        &[],
        Some(&body),
    )
    .await?;
    let page: ApiPage<LibraryPlaylist> = parse(response.ok_or(Error::EmptyResponse)?)?;
    first(page)
}

/// Adds tracks to the end of a library playlist. One request holds all tracks, in the given order.
///
/// Documentation: <https://developer.apple.com/documentation/applemusicapi/add-tracks-to-a-library-playlist>
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `tracks` is empty. Returns an error if the request fails.
pub async fn add_to_playlist(
    transport: &impl ApiTransport,
    playlist_id: &LibraryPlaylistId,
    tracks: &[Track],
) -> Result<()> {
    if tracks.is_empty() {
        return Err(invalid("no track to add".to_owned()));
    }
    let path = format!(
        "/v1/me/library/playlists/{}/tracks",
        segment("ID", playlist_id.as_str())?
    );
    let body = json!({ "data": tracks_json(tracks) });
    send(transport, WriteMethod::Post, &path, &[], Some(&body)).await?;
    Ok(())
}

/// Adds songs, albums, and playlists to the library. One request holds all items. Apple ignores
/// an ID that it cannot add.
///
/// Documentation: <https://developer.apple.com/documentation/applemusicapi/add-a-resource-to-a-library>
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `items` is empty or holds an artist. Returns an error if
/// the request fails.
pub async fn add_to_library(transport: &impl ApiTransport, items: &[CatalogItem]) -> Result<()> {
    if items.is_empty() {
        return Err(invalid("no item to add".to_owned()));
    }
    for item in items {
        item.reject_artist("adding to the library")?;
    }
    let owned: Vec<(String, String)> = group_by_kind(items)?
        .into_iter()
        .map(|(kind, group)| (format!("ids[{kind}]"), join_ids(&group)))
        .collect();
    let query: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    send(transport, WriteMethod::Post, "/v1/me/library", &query, None).await?;
    Ok(())
}

/// Adds one item to the favorites. Apple takes one type for each request.
///
/// Documentation: <https://developer.apple.com/documentation/applemusicapi/add-resource-to-favorites>
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn add_to_favorites(transport: &impl ApiTransport, item: &CatalogItem) -> Result<()> {
    let key = format!("ids[{}]", item.kind());
    let id = segment("ID", item.id())?;
    send(
        transport,
        WriteMethod::Post,
        "/v1/me/favorites",
        &[(&key, id)],
        None,
    )
    .await?;
    Ok(())
}

/// Removes one item from the favorites. Apple has no remove-favorite endpoint, but it removes the
/// favorite when the rating goes away (decision L14).
///
/// For a song, an album, and a playlist this is [`remove_rating`]. For an artist it is an
/// undocumented call (decision L19). That call also stores a dislike on the artist.
///
/// Documentation: see [`remove_rating`].
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn remove_from_favorites(
    transport: &impl ApiTransport,
    item: &CatalogItem,
) -> Result<()> {
    match item {
        CatalogItem::Artist(id) => {
            // Exception L19 to the documented-endpoints rule, accepted by the user. Apple has no
            // artist rating page. This call is the only way to clear an artist favorite, and it
            // can stop working at any time.
            let path = format!("/v1/me/ratings/artists/{}", segment("ID", id.as_str())?);
            put_rating(transport, &path, Rating::Dislike).await
        }
        CatalogItem::Song(_) | CatalogItem::Album(_) | CatalogItem::Playlist(_) => {
            remove_rating(transport, item).await
        }
    }
}

/// Reads if each item is a favorite. The request is live and never uses the cache (decision L18).
/// The result has one entry for each item. An item that Apple does not return is not a favorite.
///
/// Documentation: the `inFavorites` attribute of
/// <https://developer.apple.com/documentation/applemusicapi/songs>,
/// <https://developer.apple.com/documentation/applemusicapi/albums>,
/// <https://developer.apple.com/documentation/applemusicapi/artists>, and
/// <https://developer.apple.com/documentation/applemusicapi/playlists>. Apple sends the attribute
/// only when the request has `extend=inFavorites`.
///
/// # Errors
///
/// Returns an error if a request fails or a response is not valid.
pub async fn favorite_status(
    transport: &impl ApiTransport,
    storefront: &str,
    items: &[CatalogItem],
) -> Result<HashMap<CatalogItem, bool>> {
    let storefront = segment("storefront", storefront)?;
    let mut status = HashMap::new();
    for (kind, group) in group_by_kind(items)? {
        let path = format!("/v1/catalog/{storefront}/{kind}");
        let ids = join_ids(&group);
        let query = [("ids", ids.as_str()), ("extend", "inFavorites")];
        let page: ApiPage<FavoriteEntry> = parse(transport.get_uncached(&path, &query).await?)?;
        let favorites: Vec<String> = page
            .data
            .into_iter()
            .filter(|entry| entry.attributes.in_favorites)
            .map(|entry| entry.id)
            .collect();
        for item in group {
            let is_favorite = favorites.iter().any(|id| id == item.id());
            status.insert(item.clone(), is_favorite);
        }
    }
    Ok(status)
}

/// Reads the rating of an item. Returns `None` if the item has no rating.
///
/// Apple answers `404` when there is no rating, and `400` for an album. So an album always gives
/// `None`, without a request. Use [`favorite_status`] for the star of an album.
///
/// Documentation:
/// <https://developer.apple.com/documentation/applemusicapi/get-a-personal-content-rating-4k9c0>
/// (songs) and
/// <https://developer.apple.com/documentation/applemusicapi/get-a-personal-content-rating-6wib7>
/// (playlists).
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] for an artist. Returns an error if the request fails or the
/// response is not valid.
pub async fn rating(transport: &impl ApiTransport, item: &CatalogItem) -> Result<Option<Rating>> {
    match item {
        CatalogItem::Album(_) => return Ok(None),
        CatalogItem::Artist(_) => return item.reject_artist("reading a rating").map(|()| None),
        CatalogItem::Song(_) | CatalogItem::Playlist(_) => {}
    }
    let path = rating_path(item)?;
    let value = match transport.get(&path, &[]).await {
        Ok(value) => value,
        Err(Error::Transport(TransportError::NotFound { .. })) => return Ok(None),
        Err(error) => return Err(error),
    };
    let entry: RatingEntry = first(parse(value)?)?;
    Ok(match entry.attributes.value {
        1 => Some(Rating::Love),
        -1 => Some(Rating::Dislike),
        _ => None,
    })
}

/// Sets the rating of a song, an album, or a playlist. A new rating replaces the old one.
///
/// `Love` is the same state as a favorite (decision L7).
///
/// Documentation:
/// <https://developer.apple.com/documentation/applemusicapi/add-a-personal-content-rating-33dop>
/// (songs),
/// <https://developer.apple.com/documentation/applemusicapi/add-a-personal-content-rating-4zesn>
/// (albums), and
/// <https://developer.apple.com/documentation/applemusicapi/add-a-personal-content-rating-76n4r>
/// (playlists).
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] for an artist. Returns an error if the request fails.
pub async fn set_rating(
    transport: &impl ApiTransport,
    item: &CatalogItem,
    rating: Rating,
) -> Result<()> {
    item.reject_artist("setting a rating")?;
    put_rating(transport, &rating_path(item)?, rating).await
}

/// Removes the rating of a song, an album, or a playlist. This also removes the favorite.
///
/// Documentation:
/// <https://developer.apple.com/documentation/applemusicapi/delete-a-personal-content-rating-3a3a2>
/// (songs),
/// <https://developer.apple.com/documentation/applemusicapi/delete-a-personal-content-rating-863sx>
/// (albums), and
/// <https://developer.apple.com/documentation/applemusicapi/delete-a-personal-content-rating-mv3a>
/// (playlists).
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] for an artist. Returns an error if the request fails.
pub async fn remove_rating(transport: &impl ApiTransport, item: &CatalogItem) -> Result<()> {
    item.reject_artist("removing a rating")?;
    let path = rating_path(item)?;
    send(transport, WriteMethod::Delete, &path, &[], None).await?;
    Ok(())
}

/// Reads if a song, an album, or a playlist is in the library. The request is live and never uses
/// the cache (decision L17). Use it for one item at a time.
///
/// Documentation: the `library` relationship of
/// <https://developer.apple.com/documentation/applemusicapi/get-a-catalog-song>,
/// <https://developer.apple.com/documentation/applemusicapi/albums>, and
/// <https://developer.apple.com/documentation/applemusicapi/playlists>. An empty relationship
/// means "not in the library". A new library item can show after a delay.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] for an artist. Returns an error if the request fails or the
/// response is not valid.
pub async fn library_status(
    transport: &impl ApiTransport,
    storefront: &str,
    item: &CatalogItem,
) -> Result<bool> {
    item.reject_artist("the library status")?;
    let path = catalog_path(storefront, item.kind(), item.id())?;
    let value = transport
        .get_uncached(&path, &[("include", "library")])
        .await?;
    let probe: LibraryProbe = first(parse(value)?)?;
    let library = probe
        .relationships
        .and_then(|relationships| relationships.library);
    Ok(library.is_some_and(|relationship| !relationship.data.is_empty()))
}

/// One resource of an `inFavorites` response.
#[derive(Deserialize)]
struct FavoriteEntry {
    id: String,
    #[serde(default)]
    attributes: FavoriteAttributes,
}

#[derive(Default, Deserialize)]
struct FavoriteAttributes {
    #[serde(default, rename = "inFavorites")]
    in_favorites: bool,
}

/// One resource of a rating response.
#[derive(Deserialize)]
struct RatingEntry {
    attributes: RatingAttributes,
}

#[derive(Deserialize)]
struct RatingAttributes {
    value: i64,
}

/// One catalog resource, read for its `library` relationship.
#[derive(Deserialize)]
struct LibraryProbe {
    relationships: Option<LibraryProbeRelationships>,
}

#[derive(Deserialize)]
struct LibraryProbeRelationships {
    library: Option<Relationship<ResourceRef>>,
}

/// Sends a write request.
async fn send(
    transport: &impl ApiTransport,
    method: WriteMethod,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
) -> Result<Option<Value>> {
    transport
        .write(WriteRequest {
            method,
            path,
            query,
            body,
        })
        .await
}

/// Sends a `PUT` that sets a rating.
async fn put_rating(transport: &impl ApiTransport, path: &str, rating: Rating) -> Result<()> {
    let body = json!({ "type": "rating", "attributes": { "value": rating.value() } });
    send(transport, WriteMethod::Put, path, &[], Some(&body)).await?;
    Ok(())
}

fn rating_path(item: &CatalogItem) -> Result<String> {
    Ok(format!(
        "/v1/me/ratings/{}/{}",
        item.kind(),
        segment("ID", item.id())?
    ))
}

fn tracks_json(tracks: &[Track]) -> Vec<Value> {
    tracks.iter().map(Track::to_json).collect()
}

/// Groups items by kind. The groups keep the order of the first item of each kind.
fn group_by_kind(items: &[CatalogItem]) -> Result<Vec<(&'static str, Vec<&CatalogItem>)>> {
    let mut groups: Vec<(&'static str, Vec<&CatalogItem>)> = Vec::new();
    for item in items {
        segment("ID", item.id())?;
        match groups.iter_mut().find(|(kind, _)| *kind == item.kind()) {
            Some((_, group)) => group.push(item),
            None => groups.push((item.kind(), vec![item])),
        }
    }
    Ok(groups)
}

fn join_ids(items: &[&CatalogItem]) -> String {
    let ids: Vec<&str> = items.iter().map(|item| item.id()).collect();
    ids.join(",")
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(Error::Parse)
}
