// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// The traits of `mpris-server` make every method `async`, also when it does not wait.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "the trait methods are async"
)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mpris_server::zbus::{self, fdo};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, RootInterface, Time,
    TrackId, Volume,
};
use ringo_core::artwork::ArtworkStore;
use ringo_core::state::{AppState, Message};
use ringo_engine::protocol::PlaybackState;
use tokio::sync::{mpsc, watch};

use crate::callbacks::Callbacks;
use crate::capabilities::Capabilities;
use crate::mapping;
use crate::position::seek_target;

/// Name of the application for the desktop shell.
const IDENTITY: &str = "RingoMusic";

/// Name of the desktop file without the `.desktop` suffix.
const DESKTOP_ENTRY: &str = "io.github.MedaiP90.RingoMusic";

/// The D-Bus object. It answers property reads from the latest state and
/// turns method calls into messages for the core.
pub(crate) struct Player {
    state: watch::Receiver<AppState>,
    inbox: mpsc::Sender<Message>,
    callbacks: Callbacks,
    artwork: ArtworkStore,
}

impl Player {
    pub(crate) fn new(
        state: watch::Receiver<AppState>,
        inbox: mpsc::Sender<Message>,
        callbacks: Callbacks,
        artwork: ArtworkStore,
    ) -> Self {
        Self {
            state,
            inbox,
            callbacks,
            artwork,
        }
    }

    /// Reads a value from the latest state. The lock is not held after the call.
    fn read<T>(&self, read: impl FnOnce(&AppState) -> T) -> T {
        read(&self.state.borrow())
    }

    async fn send(&self, message: Message) -> fdo::Result<()> {
        self.inbox
            .send(message)
            .await
            .map_err(|_| fdo::Error::Failed("the application is shutting down".to_owned()))
    }
}

impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        self.callbacks.raise();
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.callbacks.quit();
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Err(not_supported("fullscreen"))
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok(IDENTITY.to_owned())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(DESKTOP_ENTRY.to_owned())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(Vec::new())
    }
}

impl PlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        self.send(Message::Next).await
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.send(Message::Previous).await
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.send(Message::Pause).await
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let playing = self.read(|state| state.playback.state == PlaybackState::Playing);
        self.send(if playing {
            Message::Pause
        } else {
            Message::Play
        })
        .await
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.send(Message::Stop).await
    }

    async fn play(&self) -> fdo::Result<()> {
        self.send(Message::Play).await
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let target = self.read(|state| {
            let duration = mapping::current_duration(state)?;
            Some(seek_target(
                state.playback.position,
                offset.as_micros(),
                duration,
            ))
        });
        match target {
            None => Ok(()),
            Some(Some(position)) => self.send(Message::Seek(position)).await,
            // A seek after the end acts like "next" (MPRIS specification).
            Some(None) => self.send(Message::Next).await,
        }
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        let target = self.read(|state| {
            let duration = mapping::current_duration(state)?;
            let current = mapping::current_track_id(state);
            let micros = u64::try_from(position.as_micros()).ok()?;
            let position = Duration::from_micros(micros);
            (track_id == current && position <= duration).then_some(position)
        });
        // The specification says: ignore a stale track ID or a position outside the item.
        match target {
            Some(position) => self.send(Message::Seek(position)).await,
            None => Ok(()),
        }
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("OpenUri".to_owned()))
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(self.read(|state| mapping::playback_status(state.playback.state)))
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(self.read(|state| mapping::loop_status(state.queue.repeat())))
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
        Ok(self
            .send(Message::SetRepeat(mapping::repeat_mode(loop_status)))
            .await?)
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Err(not_supported("rate"))
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.read(|state| state.queue.shuffle_enabled()))
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        let message = Message::SetShuffle {
            enabled: shuffle,
            seed: random_seed(),
        };
        Ok(self.send(message).await?)
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.read(|state| mapping::cached_metadata(state, &self.artwork)))
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.read(|state| f64::from(state.playback.volume)))
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        // The core limits the value to 0.0 to 1.0. The cast loses precision that nobody hears.
        #[expect(clippy::cast_possible_truncation, reason = "volume is limited to 0..1")]
        let volume = volume.clamp(0.0, 1.0) as f32;
        Ok(self.send(Message::SetVolume(volume)).await?)
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(self.read(|state| mapping::to_time(state.playback.position)))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(self.read(|state| Capabilities::from_state(state).can_go_next))
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(self.read(|state| Capabilities::from_state(state).can_go_previous))
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(self.read(|state| Capabilities::from_state(state).can_play))
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(self.read(|state| Capabilities::from_state(state).can_pause))
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(self.read(|state| Capabilities::from_state(state).can_seek))
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

fn not_supported(property: &str) -> zbus::Error {
    fdo::Error::NotSupported(format!("{property} cannot be changed")).into()
}

/// Returns a seed for the shuffle order. The clock is random enough for a playlist.
fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    // Only the low bits change fast, so keeping them is enough.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low 64 bits are the seed"
    )]
    let seed = nanos as u64;
    seed
}
