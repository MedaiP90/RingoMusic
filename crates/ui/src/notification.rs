// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Track-change notification.

use gettextrs::gettext;
use relm4::gtk::gio::Notification;
use ringo_core::state::NowPlaying;

/// All track notifications share this ID, so a new one replaces the old one.
pub const NOTIFICATION_ID: &str = "now-playing";

/// Builds the notification for `track`. The title is the song title. The body
/// has the artist and the album.
pub fn now_playing(track: &NowPlaying) -> Notification {
    let notification = Notification::new(&track.title);
    notification.set_body(Some(&body(&track.artist, &track.album)));
    notification
}

/// Fills the translatable body format.
fn body(artist: &str, album: &str) -> String {
    gettext("{artist} — {album}")
        .replace("{artist}", artist)
        .replace("{album}", album)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_contains_artist_and_album() {
        assert_eq!(body("Daft Punk", "Discovery"), "Daft Punk — Discovery");
    }
}
