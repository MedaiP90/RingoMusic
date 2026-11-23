// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Instant;

use mpris_server::{Server, Signal};
use ringo_core::artwork::ArtworkStore;
use ringo_core::state::{AppState, Message};
use tokio::sync::{mpsc, watch};
use tracing::warn;

use crate::callbacks::Callbacks;
use crate::error::{Error, Result};
use crate::mapping;
use crate::player::Player;
use crate::position::JumpDetector;
use crate::snapshot::Snapshot;

/// Bus name after `org.mpris.MediaPlayer2.`.
const BUS_NAME_SUFFIX: &str = "RingoMusic";

/// Publishes the player as `org.mpris.MediaPlayer2.RingoMusic` and keeps the
/// clients up to date until `state` has no sender left.
///
/// Run it on the Tokio runtime. It does not return while the application runs.
///
/// # Errors
///
/// Returns [`Error::Bus`] if the session bus is not available or the bus name
/// is taken.
pub async fn run(
    state: watch::Receiver<AppState>,
    inbox: mpsc::Sender<Message>,
    callbacks: Callbacks,
    artwork: ArtworkStore,
) -> Result<()> {
    let player = Player::new(state.clone(), inbox, callbacks, artwork.clone());
    let server = Server::new(BUS_NAME_SUFFIX, player)
        .await
        .map_err(Error::bus)?;
    publish_changes(&server, state, artwork).await;
    Ok(())
}

/// Sends `PropertiesChanged` and `Seeked` for each new state. It also downloads
/// the artwork of the current item, and sends the metadata again when the file
/// is on disk.
async fn publish_changes(
    server: &Server<Player>,
    mut state: watch::Receiver<AppState>,
    artwork: ArtworkStore,
) {
    let mut jumps = JumpDetector::default();
    let (downloaded, mut downloads) = mpsc::unbounded_channel();
    let mut requested = None;
    let first = state.borrow_and_update().clone();
    request_artwork(&first, &artwork, &downloaded, &mut requested);
    let mut published = Snapshot::of(&first, &artwork);
    loop {
        tokio::select! {
            changed = state.changed() => if changed.is_err() { break },
            _ = downloads.recv() => {}
        }
        let current = state.borrow_and_update().clone();
        request_artwork(&current, &artwork, &downloaded, &mut requested);
        let snapshot = Snapshot::of(&current, &artwork);
        let changes = snapshot.changes_since(&published);
        published = snapshot;
        if !changes.is_empty()
            && let Err(error) = server.properties_changed(changes).await
        {
            warn!(%error, "cannot send the changed MPRIS properties");
        }
        let item = current.now_playing.as_ref().map(|playing| &playing.id);
        let playing = mapping::playback_status(current.playback.state)
            == mpris_server::PlaybackStatus::Playing;
        let jump = jumps.observe(item, current.playback.position, playing, Instant::now());
        if let Some(position) = jump
            && let Err(error) = server
                .emit(Signal::Seeked {
                    position: mapping::to_time(position),
                })
                .await
        {
            warn!(%error, "cannot send the MPRIS seek signal");
        }
    }
}

/// Starts the download of the artwork of the current item, one time for each
/// URL. `downloaded` gets a message when the download ends.
fn request_artwork(
    state: &AppState,
    artwork: &ArtworkStore,
    downloaded: &mpsc::UnboundedSender<()>,
    requested: &mut Option<String>,
) {
    let url = mapping::current_artwork_url(state);
    if url == *requested {
        return;
    }
    requested.clone_from(&url);
    let Some(url) = url else { return };
    let (artwork, downloaded) = (artwork.clone(), downloaded.clone());
    tokio::spawn(async move {
        if artwork.fetch(&url).await.is_some() {
            // The receiver lives as long as the server loop. A failed send means shutdown.
            let _ = downloaded.send(());
        }
    });
}
