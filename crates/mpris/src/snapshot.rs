// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Property};
use ringo_core::artwork::ArtworkStore;
use ringo_core::state::AppState;

use crate::capabilities::Capabilities;
use crate::mapping;

/// The MPRIS properties that can change, as one value. Comparing two
/// snapshots shows which `PropertiesChanged` entries clients need.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    status: PlaybackStatus,
    loop_status: LoopStatus,
    shuffle: bool,
    metadata: Metadata,
    volume: f64,
    capabilities: Capabilities,
}

impl Snapshot {
    pub(crate) fn of(state: &AppState, artwork: &ArtworkStore) -> Self {
        Self {
            status: mapping::playback_status(state.playback.state),
            loop_status: mapping::loop_status(state.queue.repeat()),
            shuffle: state.queue.shuffle_enabled(),
            metadata: mapping::cached_metadata(state, artwork),
            volume: f64::from(state.playback.volume),
            capabilities: Capabilities::from_state(state),
        }
    }

    /// Returns the properties that differ from `previous`.
    pub(crate) fn changes_since(&self, previous: &Self) -> Vec<Property> {
        let mut changes = Vec::new();
        if self.status != previous.status {
            changes.push(Property::PlaybackStatus(self.status));
        }
        if self.loop_status != previous.loop_status {
            changes.push(Property::LoopStatus(self.loop_status));
        }
        if self.shuffle != previous.shuffle {
            changes.push(Property::Shuffle(self.shuffle));
        }
        if self.metadata != previous.metadata {
            changes.push(Property::Metadata(self.metadata.clone()));
        }
        // The volume is a float that clients compare by value, so any change counts.
        #[expect(clippy::float_cmp, reason = "a copy of the same value is equal")]
        if self.volume != previous.volume {
            changes.push(Property::Volume(self.volume));
        }
        self.push_capability_changes(previous, &mut changes);
        changes
    }

    fn push_capability_changes(&self, previous: &Self, changes: &mut Vec<Property>) {
        let (now, before) = (self.capabilities, previous.capabilities);
        if now.can_go_next != before.can_go_next {
            changes.push(Property::CanGoNext(now.can_go_next));
        }
        if now.can_go_previous != before.can_go_previous {
            changes.push(Property::CanGoPrevious(now.can_go_previous));
        }
        if now.can_play != before.can_play {
            changes.push(Property::CanPlay(now.can_play));
        }
        if now.can_pause != before.can_pause {
            changes.push(Property::CanPause(now.can_pause));
        }
        if now.can_seek != before.can_seek {
            changes.push(Property::CanSeek(now.can_seek));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ringo_core::state::EngineStatus;
    use ringo_engine::protocol::PlaybackState;

    use super::*;

    fn store() -> ArtworkStore {
        ArtworkStore::new("/nonexistent".into()).expect("client starts")
    }

    #[test]
    fn equal_snapshots_have_no_changes() {
        let snapshot = Snapshot::of(&AppState::new(), &store());
        assert_eq!(snapshot.changes_since(&snapshot.clone()), Vec::new());
    }

    #[test]
    fn position_updates_are_not_changes() {
        let before = Snapshot::of(&AppState::new(), &store());
        let mut state = AppState::new();
        state.playback.position = Duration::from_secs(5);
        assert_eq!(
            Snapshot::of(&state, &store()).changes_since(&before),
            Vec::new()
        );
    }

    #[test]
    fn playback_state_and_volume_changes_are_reported() {
        let before = Snapshot::of(&AppState::new(), &store());
        let mut state = AppState::new();
        state.playback.state = PlaybackState::Playing;
        state.playback.volume = 0.5;
        let changes = Snapshot::of(&state, &store()).changes_since(&before);
        assert_eq!(
            changes,
            [
                Property::PlaybackStatus(PlaybackStatus::Playing),
                Property::Volume(0.5)
            ]
        );
    }

    #[test]
    fn a_ready_engine_without_an_item_changes_nothing() {
        let before = Snapshot::of(&AppState::new(), &store());
        let mut state = AppState::new();
        state.engine = EngineStatus::Ready;
        assert_eq!(
            Snapshot::of(&state, &store()).changes_since(&before),
            Vec::new()
        );
    }
}
