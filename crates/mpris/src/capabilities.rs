// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use ringo_core::state::{AppState, EngineStatus};

/// The MPRIS capability flags. They tell clients which controls to show.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag for each MPRIS property"
)]
pub(crate) struct Capabilities {
    pub(crate) can_go_next: bool,
    pub(crate) can_go_previous: bool,
    pub(crate) can_play: bool,
    pub(crate) can_pause: bool,
    pub(crate) can_seek: bool,
}

impl Capabilities {
    /// Derives the flags from the state. Without an item or without a ready
    /// engine, no control works.
    pub(crate) fn from_state(state: &AppState) -> Self {
        let has_item = state.queue.current().is_some() || state.now_playing.is_some();
        let usable = has_item && state.engine == EngineStatus::Ready;
        Self {
            // "Previous" restarts the item at the start of the queue, so it always works.
            can_go_next: usable,
            can_go_previous: usable,
            can_play: usable,
            can_pause: usable,
            can_seek: usable,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use ringo_core::queue::QueueItem;
    use ringo_engine::ItemId;

    use super::*;

    fn state_with_item(engine: EngineStatus) -> AppState {
        let mut state = AppState::new();
        state.engine = engine;
        let item = QueueItem {
            id: ItemId::new("1"),
            title: "Song".to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            duration: Duration::from_secs(60),
            artwork_template: None,
        };
        let _ = Arc::make_mut(&mut state.queue).replace(vec![item], 0);
        state
    }

    #[test]
    fn controls_are_off_without_an_item() {
        let mut state = AppState::new();
        state.engine = EngineStatus::Ready;
        let capabilities = Capabilities::from_state(&state);
        assert!(!capabilities.can_play);
        assert!(!capabilities.can_go_next);
    }

    #[test]
    fn controls_are_off_while_the_engine_is_not_ready() {
        let state = state_with_item(EngineStatus::Starting);
        assert!(!Capabilities::from_state(&state).can_play);
    }

    #[test]
    fn controls_are_on_with_an_item_and_a_ready_engine() {
        let state = state_with_item(EngineStatus::Ready);
        let capabilities = Capabilities::from_state(&state);
        assert!(capabilities.can_play);
        assert!(capabilities.can_pause);
        assert!(capabilities.can_seek);
        assert!(capabilities.can_go_next);
        assert!(capabilities.can_go_previous);
    }
}
