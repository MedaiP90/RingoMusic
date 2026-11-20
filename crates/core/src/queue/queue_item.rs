// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use ringo_engine::ItemId;

/// One song in the queue, with the data that the UI shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueItem {
    /// ID that the engine plays.
    pub id: ItemId,
    /// Title of the song.
    pub title: String,
    /// Name of the artist.
    pub artist: String,
    /// Title of the album.
    pub album: String,
    /// Length of the song.
    pub duration: Duration,
    /// Artwork URL template with `{w}`, `{h}`, and `{f}` placeholders.
    pub artwork_template: Option<String>,
}
