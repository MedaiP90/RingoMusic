// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Temporary debug action: plays the songs of `RINGO_DEBUG_PLAY`.
// ponytail: remove this file in prompt 25, when the UI can start playback.

use std::env;
use std::time::Duration;

use ringo_core::queue::QueueItem;
use ringo_core::runner::CoreHandle;
use ringo_core::state::{AuthState, EngineStatus, Message};
use ringo_engine::ItemId;

/// Environment variable with song IDs, separated by commas.
const VARIABLE: &str = "RINGO_DEBUG_PLAY";

/// Starts playback of the songs in `RINGO_DEBUG_PLAY`, when the engine is
/// ready and the user is signed in. It does nothing if the variable is not set.
pub fn spawn(core: CoreHandle) {
    let Ok(ids) = env::var(VARIABLE) else {
        return;
    };
    let items: Vec<QueueItem> = ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(debug_item)
        .collect();
    relm4::spawn(async move {
        let mut state = core.subscribe();
        // The result holds a lock, so only its success is kept.
        let ready = state
            .wait_for(|state| {
                state.engine == EngineStatus::Ready
                    && matches!(state.auth, AuthState::SignedIn { .. })
            })
            .await
            .is_ok();
        if ready {
            core.send(Message::ReplaceQueue { items, start: 0 }).await;
        }
    });
}

/// The engine reports the real title, artist, and artwork when the song plays.
fn debug_item(id: &str) -> QueueItem {
    QueueItem {
        id: ItemId::new(id),
        title: id.to_owned(),
        artist: String::new(),
        album: String::new(),
        duration: Duration::ZERO,
        artwork_template: None,
    }
}
