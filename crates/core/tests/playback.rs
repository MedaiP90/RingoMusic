// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Plays queues through `Core` with `MockEngine`. No real engine runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ringo_core::queue::QueueItem;
use ringo_core::runner::{Core, CoreHandle};
use ringo_core::state::{AppState, AuthState, EngineStatus, Message};
use ringo_core::{EngineCall, MockEngine};
use ringo_engine::protocol::{PlaybackState, RepeatMode};
use ringo_engine::{EngineEvent, ItemId};
use tokio::sync::watch;

/// Time limit for a state change. The tests do not wait this long.
const WAIT_LIMIT: Duration = Duration::from_secs(5);

struct Fixture {
    engine: Arc<MockEngine>,
    handle: CoreHandle,
    snapshots: watch::Receiver<AppState>,
}

impl Fixture {
    /// Starts a runner on a mock engine that already sent `engine.ready`.
    async fn start() -> Self {
        let engine = Arc::new(MockEngine::new());
        let (core, handle) = Core::new(Arc::clone(&engine) as _);
        tokio::spawn(core.run());
        let mut fixture = Self {
            snapshots: handle.subscribe(),
            engine,
            handle,
        };
        fixture.engine.emit(EngineEvent::Ready);
        fixture
            .wait_for(|state| matches!(state.auth, AuthState::SignedIn { .. }))
            .await;
        fixture
    }

    /// Replaces the queue and waits until the runner has applied it, so that
    /// later engine events see the new queue.
    async fn replace_queue(&mut self, ids: &[&str], start: usize) {
        let items = queue_items(ids);
        let count = items.len();
        self.send(Message::ReplaceQueue { items, start }).await;
        self.wait_for(|state| state.queue.items().len() == count)
            .await;
    }

    async fn send(&self, message: Message) {
        self.handle.send(message).await;
    }

    /// Waits until the snapshot satisfies `condition`.
    async fn wait_for(&mut self, condition: impl FnMut(&AppState) -> bool) -> AppState {
        let wait = self.snapshots.wait_for(condition);
        let snapshot = tokio::time::timeout(WAIT_LIMIT, wait)
            .await
            .expect("the state did not reach the condition in time")
            .expect("the runner is running");
        snapshot.clone()
    }

    /// The engine plays the item at `index` of a queue.
    async fn engine_plays(&mut self, id: &str, index: usize) {
        self.engine.emit(EngineEvent::NowPlaying {
            id: ItemId::new(id),
            title: format!("title {id}"),
            artist: "artist".to_owned(),
            album: "album".to_owned(),
            duration: Duration::from_secs(3),
            artwork_url: None,
            album_id: None,
            queue_index: index,
        });
        self.wait_for(|state| {
            state
                .now_playing
                .as_ref()
                .is_some_and(|now| now.id.as_str() == id)
        })
        .await;
    }
}

fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/events")
        .join(format!("{name}.jsonl"))
}

fn queue_items(ids: &[&str]) -> Vec<QueueItem> {
    ids.iter()
        .map(|id| QueueItem {
            id: ItemId::new(*id),
            title: format!("title {id}"),
            artist: "artist".to_owned(),
            album: "album".to_owned(),
            duration: Duration::from_secs(3),
            artwork_template: None,
        })
        .collect()
}

fn ids(ids: &[&str]) -> Vec<ItemId> {
    ids.iter().map(|id| ItemId::new(*id)).collect()
}

fn current_id(state: &AppState) -> &str {
    state
        .queue
        .current()
        .expect("queue has a current item")
        .id
        .as_str()
}

/// Returns `true` when the current item of the queue has the ID `id`.
fn is_current(state: &AppState, id: &str) -> bool {
    state
        .queue
        .current()
        .is_some_and(|item| item.id.as_str() == id)
}

#[tokio::test]
async fn song_plays_to_the_end_with_repeat_off() {
    let mut fixture = Fixture::start().await;
    fixture.replace_queue(&["s1"], 0).await;
    fixture.engine.emit_script(&script("song_plays_to_end"));
    let state = fixture
        .wait_for(|state| state.playback.state == PlaybackState::Ended)
        .await;
    assert_eq!(state.playback.position, Duration::from_secs(3));
    assert_eq!(current_id(&state), "s1");
    let calls = fixture.engine.calls();
    assert!(calls.contains(&EngineCall::SetQueue {
        items: ids(&["s1"]),
        start: 0
    }));
    assert!(calls.contains(&EngineCall::Play));
}

#[tokio::test]
async fn queue_plays_to_the_end_with_repeat_off() {
    let mut fixture = Fixture::start().await;
    fixture.replace_queue(&["s1", "s2", "s3"], 0).await;
    for (index, id) in ["s1", "s2", "s3"].into_iter().enumerate() {
        fixture.engine_plays(id, index).await;
    }
    fixture.engine.emit(EngineEvent::State {
        state: PlaybackState::Ended,
    });
    let state = fixture
        .wait_for(|state| state.playback.state == PlaybackState::Ended)
        .await;
    assert_eq!(current_id(&state), "s3");
    assert_eq!(state.queue.history().count(), 2);
}

#[tokio::test]
async fn queue_wraps_to_the_first_item_with_repeat_all() {
    let mut fixture = Fixture::start().await;
    fixture.send(Message::SetRepeat(RepeatMode::All)).await;
    fixture.replace_queue(&["s1", "s2"], 0).await;
    fixture.engine_plays("s1", 0).await;
    fixture.engine_plays("s2", 1).await;
    fixture.engine_plays("s1", 0).await;
    let state = fixture
        .wait_for(|state| state.queue.history().count() == 2)
        .await;
    assert_eq!(current_id(&state), "s1");
    assert_eq!(state.queue.repeat(), RepeatMode::All);
    assert!(
        fixture
            .engine
            .calls()
            .contains(&EngineCall::SetRepeat(RepeatMode::All))
    );
}

#[tokio::test]
async fn next_and_previous_move_the_queue_and_call_the_engine() {
    let mut fixture = Fixture::start().await;
    fixture.replace_queue(&["s1", "s2", "s3"], 0).await;
    fixture.send(Message::Next).await;
    fixture.wait_for(|state| is_current(state, "s2")).await;
    fixture.send(Message::Next).await;
    fixture.wait_for(|state| is_current(state, "s3")).await;
    fixture.send(Message::Previous).await;
    fixture.wait_for(|state| is_current(state, "s2")).await;
    // The engine is the last to answer, so wait for the calls.
    fixture.send(Message::Pause).await;
    let calls = tokio::time::timeout(WAIT_LIMIT, async {
        loop {
            let calls = fixture.engine.calls();
            if calls.contains(&EngineCall::Pause) {
                break calls;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the engine got the pause call");
    let moves: Vec<_> = calls
        .iter()
        .filter(|call| matches!(call, EngineCall::Next | EngineCall::PlayNext(_)))
        .cloned()
        .collect();
    // The previous item goes through `play_next` and `next`, not `previous`.
    assert_eq!(
        moves,
        [
            EngineCall::Next,
            EngineCall::Next,
            EngineCall::PlayNext(ids(&["s2"])),
            EngineCall::Next
        ]
    );
    assert!(!calls.contains(&EngineCall::Previous));
}

#[tokio::test]
async fn volume_goes_to_the_state_and_the_engine() {
    let mut fixture = Fixture::start().await;
    fixture.send(Message::SetVolume(0.25)).await;
    let state = fixture
        .wait_for(|state| (state.playback.volume - 0.25).abs() < f32::EPSILON)
        .await;
    assert!((state.playback.volume - 0.25).abs() < f32::EPSILON);
    tokio::time::timeout(WAIT_LIMIT, async {
        while !fixture
            .engine
            .calls()
            .contains(&EngineCall::SetVolume(0.25))
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the engine got the volume");
}

#[tokio::test]
async fn engine_restart_sets_the_queue_again() {
    let mut fixture = Fixture::start().await;
    fixture.replace_queue(&["s1", "s2"], 0).await;
    fixture.engine_plays("s2", 1).await;
    fixture.engine.emit_script(&script("engine_restart"));
    fixture
        .wait_for(|state| state.engine == EngineStatus::Ready)
        .await;
    let restored = EngineCall::SetQueue {
        items: ids(&["s1", "s2"]),
        start: 1,
    };
    tokio::time::timeout(WAIT_LIMIT, async {
        while !fixture.engine.calls().contains(&restored) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the queue was set again after the restart");
}

#[tokio::test]
async fn upcoming_changes_use_small_edits_during_playback() {
    let mut fixture = Fixture::start().await;
    fixture.replace_queue(&["s1", "s2"], 0).await;
    fixture.engine_plays("s1", 0).await;
    fixture.engine.emit(EngineEvent::State {
        state: PlaybackState::Playing,
    });
    fixture
        .wait_for(|state| state.playback.state == PlaybackState::Playing)
        .await;
    fixture.send(Message::PlayNext(queue_items(&["x"]))).await;
    fixture.send(Message::Append(queue_items(&["y"]))).await;
    fixture
        .send(Message::Remove {
            index: 2,
            id: ItemId::new("s2"),
        })
        .await;
    tokio::time::timeout(WAIT_LIMIT, async {
        while !fixture.engine.calls().contains(&EngineCall::Remove(2)) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the engine got the edits");
    let calls = fixture.engine.calls();
    let sets = calls
        .iter()
        .filter(|call| matches!(call, EngineCall::SetQueue { .. }))
        .count();
    assert_eq!(sets, 1, "only the first queue goes as a full queue");
    let edits: Vec<_> = calls
        .into_iter()
        .filter(|call| {
            matches!(
                call,
                EngineCall::PlayNext(_) | EngineCall::PlayLater(_) | EngineCall::Remove(_)
            )
        })
        .collect();
    assert_eq!(
        edits,
        [
            EngineCall::PlayNext(ids(&["x"])),
            EngineCall::PlayLater(ids(&["y"])),
            EngineCall::Remove(2),
        ]
    );
}

#[tokio::test]
async fn shuffle_on_and_off_in_the_middle_keeps_the_engine_in_sync() {
    let mut fixture = Fixture::start().await;
    fixture
        .replace_queue(&["s1", "s2", "s3", "s4", "s5"], 2)
        .await;
    fixture.engine_plays("s3", 2).await;
    fixture
        .send(Message::SetShuffle {
            enabled: true,
            seed: 11,
        })
        .await;
    let shuffled = fixture
        .wait_for(|state| state.queue.shuffle_enabled())
        .await;
    // The engine keeps its two old items before the current item, so its next
    // item has the engine index 3, and the Rust index 1.
    let next_id = shuffled.queue.playback_order()[1].clone();
    fixture.engine_plays(next_id.as_str(), 3).await;
    let state = fixture
        .wait_for(|state| state.queue.current_playback_index() == Some(1))
        .await;
    assert_eq!(current_id(&state), next_id.as_str());
    fixture
        .send(Message::SetShuffle {
            enabled: false,
            seed: 0,
        })
        .await;
    fixture
        .wait_for(|state| !state.queue.shuffle_enabled())
        .await;
    let calls = fixture.engine.calls();
    let sets = calls
        .iter()
        .filter(|call| matches!(call, EngineCall::SetQueue { .. }))
        .count();
    assert_eq!(sets, 1, "shuffle changes never send the full queue");
}

#[tokio::test]
async fn previous_after_shuffle_off_plays_the_item_before_in_user_order() {
    let mut fixture = Fixture::start().await;
    fixture
        .replace_queue(&["s1", "s2", "s3", "s4", "s5"], 2)
        .await;
    fixture.engine_plays("s3", 2).await;
    fixture
        .send(Message::SetShuffle {
            enabled: true,
            seed: 11,
        })
        .await;
    let shuffled = fixture
        .wait_for(|state| state.queue.shuffle_enabled())
        .await;
    let next_id = shuffled.queue.playback_order()[1].clone();
    fixture.engine_plays(next_id.as_str(), 3).await;
    fixture
        .send(Message::SetShuffle {
            enabled: false,
            seed: 0,
        })
        .await;
    let ordered = fixture
        .wait_for(|state| !state.queue.shuffle_enabled())
        .await;
    let position = ordered
        .queue
        .current_playback_index()
        .expect("queue has a current item");
    let expected = if position == 0 {
        None
    } else {
        Some(ordered.queue.items()[position - 1].id.clone())
    };
    fixture.send(Message::Previous).await;
    let Some(expected) = expected else {
        return;
    };
    let state = fixture
        .wait_for(|state| is_current(state, expected.as_str()))
        .await;
    assert_eq!(state.queue.current_playback_index(), Some(position - 1));
    tokio::time::timeout(WAIT_LIMIT, async {
        while !fixture
            .engine
            .calls()
            .contains(&EngineCall::PlayNext(vec![expected.clone()]))
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the engine got the previous item");
    assert!(!fixture.engine.calls().contains(&EngineCall::Previous));
}
