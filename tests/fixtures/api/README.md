# API fixtures

Recorded Apple Music API responses. `FixtureTransport` (crate `ringo-am-api`)
reads them in tests. Each file is the raw JSON of one `api.get` response.

## Privacy rule

This repository is public. Library and recommendation responses contain
personal data. Before you commit a fixture:

- Replace personal names with neutral values. This includes the names of your
  own playlists, display names, and curator names of private playlists.
- Check the whole file, not only the `name` fields. Search for your name and
  your Apple ID.
- Do not commit tokens, cookies, or storefront-specific account data.

Catalog responses are public data. They need no change.

## How to record

Build and configure the engine as `ringo-cli --help` explains. Sign in once
with `ringo-cli signin`. Then run each command from the repository root. The
commands use well-known catalog items. You can use other IDs.

`ringo-cli api-get` replaces `{storefront}` in the path.

```sh
F=tests/fixtures/api
```

### Catalog

```sh
ringo-cli api-get '/v1/catalog/{storefront}/search' term=radiohead types=songs,albums,artists,playlists limit=5 > $F/catalog_search.json
ringo-cli api-get '/v1/catalog/{storefront}/songs/1097861769' > $F/catalog_song.json
ringo-cli api-get '/v1/catalog/{storefront}/albums/1097861387' include=tracks > $F/catalog_album.json
ringo-cli api-get '/v1/catalog/{storefront}/artists/657515' views=top-songs,full-albums > $F/catalog_artist.json
ringo-cli api-get '/v1/catalog/{storefront}/playlists/pl.f4d106fed2bd41149aaacabb233eb5eb' include=tracks > $F/catalog_playlist.json
```

### Library (personal data: apply the privacy rule)

```sh
ringo-cli api-get /v1/me/library/playlists limit=25 > $F/library_playlists.json
ringo-cli api-get /v1/me/library/albums limit=25 > $F/library_albums.json
ringo-cli api-get /v1/me/library/artists limit=25 > $F/library_artists.json
ringo-cli api-get /v1/me/library/songs limit=25 > $F/library_songs.json
```

### Top results and library search

The library search response holds personal data: apply the privacy rule.
Top results are not a type. Add `with=topResults` to the search request.

```sh
ringo-cli api-get '/v1/catalog/{storefront}/search' term=radiohead types=songs,albums,artists,playlists limit=5 with=topResults > $F/catalog_search_top.json
ringo-cli api-get /v1/me/library/search term=<a word from your library> types=library-songs,library-albums,library-artists,library-playlists limit=5 > $F/library_search.json
```

### Recommendations (personal data: apply the privacy rule)

```sh
ringo-cli api-get /v1/me/recommendations > $F/recommendations.json
```

### New and Made For You pages

The New page is public editorial data (the fixture is large, about 1.6 MB).
The Made For You response is personal: apply the privacy rule. Replace the
playlist ID, `playParams.versionHash`, and `dateAdded` by hand.

```sh
ringo-cli api-get '/v1/editorial/{storefront}/groupings' name=music platform=web tabs=subscriber 'format[resources]=map' 'art[url]=c,f' 'extend=artistUrl,editorialArtwork,plainEditorialNotes' 'extend[station-events]=editorialVideo' 'fields[albums]=artistName,artistUrl,artwork,contentRating,editorialArtwork,plainEditorialNotes,name,playParams,releaseDate,url,trackCount' 'fields[artists]=name,url,artwork,editorialArtwork,genreNames,plainEditorialNotes' 'include[albums]=artists' 'include[music-videos]=artists' 'include[songs]=artists' 'include[stations]=events,radio-show' 'omit[resource:artists]=autos' 'relate[songs]=albums' l=it > $F/editorial_new.json
ringo-cli api-get /v1/me/library/playlists 'filter[featured]=made-for-you' 'format[resources]=map' 'include[library-playlists]=catalog' 'art[url]=f' 'fields[library-playlists]=artwork,dateAdded,name,playParams' 'fields[playlists]=lastModifiedDate' sort=type platform=web l=it > $F/made_for_you.json
```

### Broken item (lenient parsing test)

1. Copy `catalog_search.json` to `catalog_search_broken_item.json`.
2. Damage one item by hand. For example, change the `attributes` of one song
   to a string, or delete its `id`. Keep the other items valid.
3. Keep the JSON valid. Only the one item is wrong.

## Files

| File | Request |
|---|---|
| `catalog_search.json` | `/v1/catalog/{storefront}/search` |
| `catalog_song.json` | `/v1/catalog/{storefront}/songs/1097861769` |
| `catalog_album.json` | `/v1/catalog/{storefront}/albums/1097861387` with `include=tracks` |
| `catalog_artist.json` | `/v1/catalog/{storefront}/artists/657515` with `views=top-songs,full-albums` |
| `catalog_playlist.json` | `/v1/catalog/{storefront}/playlists/pl.f4d106fed2bd41149aaacabb233eb5eb` with `include=tracks` |
| `library_playlists.json` | `/v1/me/library/playlists` |
| `library_albums.json` | `/v1/me/library/albums` |
| `library_artists.json` | `/v1/me/library/artists` |
| `library_songs.json` | `/v1/me/library/songs` |
| `catalog_search_top.json` | `/v1/catalog/{storefront}/search` with `with=topResults` |
| `library_search.json` | `/v1/me/library/search` with `types=library-songs,...` |
| `recommendations.json` | `/v1/me/recommendations` |
| `editorial_new.json` | `/v1/editorial/{storefront}/groupings` with `name=music`, `tabs=subscriber`, `format[resources]=map` |
| `made_for_you.json` | `/v1/me/library/playlists` with `filter[featured]=made-for-you`, `format[resources]=map` |
| `catalog_search_broken_item.json` | `catalog_search.json` with one damaged item |

## Hand-made files

These files come from the recorded files above. They are cut to a few items
and have a `next` link, so the pagination tests have a second page.

| File | Request |
|---|---|
| `catalog_playlist_tracks.json` | `/v1/catalog/{storefront}/playlists/{id}/tracks` |
| `library_playlist.json` | `/v1/me/library/playlists/p.example1` with `include=tracks` |
| `library_playlist_tracks.json` | `/v1/me/library/playlists/p.example1/tracks` |
| `empty_data.json` | Any detail request for an unknown ID: `{"data": []}` |

### New page with a broken item

`editorial_new_broken_item.json` is `editorial_new.json` with the `attributes`
of one album of "Novità di questa settimana" replaced by a string.
