// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Pure conversions between the core types and the MPRIS types.

use std::time::Duration;

use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Time, TrackId};
use ringo_am_api::artwork_url;
use ringo_core::state::AppState;
use ringo_engine::protocol::{PlaybackState, RepeatMode};

/// Prefix of the track object paths.
const TRACK_PATH_PREFIX: &str = "/io/github/MedaiP90/RingoMusic/track";

/// Size of the artwork that the metadata points to, in pixels.
const ARTWORK_SIZE: u32 = 512;

/// Returns the object path of the queue item at `queue_index`.
pub(crate) fn track_id(queue_index: usize) -> TrackId {
    TrackId::try_from(format!("{TRACK_PATH_PREFIX}/{queue_index}"))
        .expect("track path is a valid D-Bus object path")
}

/// Returns the track ID of the current item, or `NO_TRACK` without an item.
pub(crate) fn current_track_id(state: &AppState) -> TrackId {
    if current_track(state).is_none() {
        return TrackId::NO_TRACK;
    }
    track_id(state.queue.current_index().unwrap_or_default())
}

/// Returns the metadata of the current item. It is empty without an item.
pub(crate) fn metadata(state: &AppState) -> Metadata {
    let Some(track) = current_track(state) else {
        return Metadata::new();
    };
    let mut metadata = Metadata::builder()
        .trackid(current_track_id(state))
        .title(track.title)
        .artist([track.artist])
        .album(track.album)
        .length(to_time(track.duration))
        .build();
    if let Some(template) = track.artwork {
        metadata.set_art_url(Some(artwork_url(template, ARTWORK_SIZE, ARTWORK_SIZE)));
    }
    metadata
}

/// Returns the length of the current item, if there is one.
pub(crate) fn current_duration(state: &AppState) -> Option<Duration> {
    current_track(state).map(|track| track.duration)
}

/// Converts a duration to MPRIS time. A value that is too big gives the maximum.
pub(crate) fn to_time(duration: Duration) -> Time {
    Time::from_micros(i64::try_from(duration.as_micros()).unwrap_or(i64::MAX))
}

pub(crate) fn playback_status(state: PlaybackState) -> PlaybackStatus {
    match state {
        PlaybackState::Playing | PlaybackState::Loading => PlaybackStatus::Playing,
        PlaybackState::Paused => PlaybackStatus::Paused,
        PlaybackState::Stopped | PlaybackState::Ended => PlaybackStatus::Stopped,
    }
}

pub(crate) fn loop_status(mode: RepeatMode) -> LoopStatus {
    match mode {
        RepeatMode::None => LoopStatus::None,
        RepeatMode::One => LoopStatus::Track,
        RepeatMode::All => LoopStatus::Playlist,
    }
}

pub(crate) fn repeat_mode(status: LoopStatus) -> RepeatMode {
    match status {
        LoopStatus::None => RepeatMode::None,
        LoopStatus::Track => RepeatMode::One,
        LoopStatus::Playlist => RepeatMode::All,
    }
}

/// The data of the item that the clients see.
struct Track<'state> {
    title: &'state str,
    artist: &'state str,
    album: &'state str,
    duration: Duration,
    artwork: Option<&'state str>,
}

/// Returns the item that the engine plays. Before the engine reports an item,
/// it is the current item of the queue (for example, a restored queue).
fn current_track(state: &AppState) -> Option<Track<'_>> {
    if let Some(playing) = &state.now_playing {
        return Some(Track {
            title: &playing.title,
            artist: &playing.artist,
            album: &playing.album,
            duration: playing.duration,
            artwork: playing.artwork_url.as_deref(),
        });
    }
    let item = state.queue.current()?;
    Some(Track {
        title: &item.title,
        artist: &item.artist,
        album: &item.album,
        duration: item.duration,
        artwork: item.artwork_template.as_deref(),
    })
}

#[cfg(test)]
mod tests {
    use ringo_core::queue::QueueItem;
    use ringo_core::state::NowPlaying;
    use ringo_engine::ItemId;

    use super::*;

    fn item(id: &str, title: &str) -> QueueItem {
        QueueItem {
            id: ItemId::new(id),
            title: title.to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            duration: Duration::from_secs(200),
            artwork_template: Some("https://example.com/{w}x{h}.{f}".to_owned()),
        }
    }

    fn state_with_queue() -> AppState {
        let mut state = AppState::new();
        let _ = state
            .queue
            .replace(vec![item("1", "First"), item("2", "Second")], 1);
        state
    }

    #[test]
    fn metadata_is_empty_without_an_item() {
        assert_eq!(metadata(&AppState::new()), Metadata::new());
        assert_eq!(current_track_id(&AppState::new()), TrackId::NO_TRACK);
    }

    #[test]
    fn metadata_describes_the_current_queue_item() {
        let metadata = metadata(&state_with_queue());
        assert_eq!(metadata.title(), Some("Second"));
        assert_eq!(metadata.artist(), Some(vec!["Artist".to_owned()]));
        assert_eq!(metadata.album(), Some("Album"));
        assert_eq!(metadata.length(), Some(Time::from_secs(200)));
        assert_eq!(
            metadata.art_url().as_deref(),
            Some("https://example.com/512x512.jpg")
        );
    }

    #[test]
    fn track_id_uses_the_queue_index() {
        let metadata = metadata(&state_with_queue());
        assert_eq!(
            metadata.trackid().map(|id| id.to_string()),
            Some("/io/github/MedaiP90/RingoMusic/track/1".to_owned())
        );
    }

    #[test]
    fn metadata_prefers_the_item_that_the_engine_plays() {
        let mut state = state_with_queue();
        state.now_playing = Some(NowPlaying {
            id: ItemId::new("1"),
            title: "Playing".to_owned(),
            artist: "Other".to_owned(),
            album: "Record".to_owned(),
            duration: Duration::from_secs(10),
            artwork_url: None,
        });
        let metadata = metadata(&state);
        assert_eq!(metadata.title(), Some("Playing"));
        assert_eq!(metadata.art_url(), None);
    }

    #[test]
    fn playback_status_maps_every_engine_state() {
        assert_eq!(
            playback_status(PlaybackState::Playing),
            PlaybackStatus::Playing
        );
        assert_eq!(
            playback_status(PlaybackState::Loading),
            PlaybackStatus::Playing
        );
        assert_eq!(
            playback_status(PlaybackState::Paused),
            PlaybackStatus::Paused
        );
        assert_eq!(
            playback_status(PlaybackState::Stopped),
            PlaybackStatus::Stopped
        );
        assert_eq!(
            playback_status(PlaybackState::Ended),
            PlaybackStatus::Stopped
        );
    }

    #[test]
    fn repeat_mode_and_loop_status_convert_both_ways() {
        for mode in [RepeatMode::None, RepeatMode::One, RepeatMode::All] {
            assert_eq!(repeat_mode(loop_status(mode)), mode);
        }
    }

    #[test]
    fn to_time_limits_a_huge_duration() {
        assert_eq!(to_time(Duration::MAX), Time::MAX);
    }
}
