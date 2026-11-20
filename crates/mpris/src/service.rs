// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Instant;

use mpris_server::{Server, Signal};
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
) -> Result<()> {
    let player = Player::new(state.clone(), inbox, callbacks);
    let server = Server::new(BUS_NAME_SUFFIX, player)
        .await
        .map_err(Error::bus)?;
    publish_changes(&server, state).await;
    Ok(())
}

/// Sends `PropertiesChanged` and `Seeked` for each new state.
async fn publish_changes(server: &Server<Player>, mut state: watch::Receiver<AppState>) {
    let mut jumps = JumpDetector::default();
    let mut published = Snapshot::of(&state.borrow_and_update());
    while state.changed().await.is_ok() {
        let current = state.borrow_and_update().clone();
        let snapshot = Snapshot::of(&current);
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
