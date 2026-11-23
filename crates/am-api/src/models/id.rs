// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fmt;

/// Defines one string-based resource ID type. The types stay separate, so the
/// compiler rejects an album ID where a song ID is needed.
macro_rules! id_type {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Returns the ID as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    )+};
}

id_type! {
    /// ID of a catalog song.
    SongId,
    /// ID of a catalog album.
    AlbumId,
    /// ID of a catalog artist.
    ArtistId,
    /// ID of a catalog playlist.
    PlaylistId,
    /// ID of a song in the library of the user.
    LibrarySongId,
    /// ID of an album in the library of the user.
    LibraryAlbumId,
    /// ID of an artist in the library of the user.
    LibraryArtistId,
    /// ID of a playlist in the library of the user.
    LibraryPlaylistId,
}
