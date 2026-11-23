// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Saves and restores the queue through `Core` with `MockEngine`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ringo_core::db::Database;
use ringo_core::queue::{Queue, QueueItem};
use ringo_core::queue_store::QueueStore;
use ringo_core::runner::{Core, CoreHandle};
use ringo_core::state::{AppState, EngineStatus, Message};
use ringo_core::{EngineCall, MockEngine};
use ringo_engine::protocol::{PlaybackState, RepeatMode};
use ringo_engine::{EngineEvent, ItemId};
use tokio::task::JoinHandle;

/// Time limit for a wait. The tests do not wait this long.
const WAIT_LIMIT: Duration = Duration::from_secs(5);

struct Running {
    engine: Arc<MockEngine>,
    handle: CoreHandle,
    task: JoinHandle<()>,
}

impl Running {
    fn start(store: &QueueStore) -> Self {
        let engine = Arc::new(MockEngine::new());
        let (core, handle) = Core::new(Arc::clone(&engine) as _);
        let task = tokio::spawn(core.with_queue_store(store.clone()).run());
        Self {
            engine,
            handle,
            task,
        }
    }

    async fn wait_for(&self, condition: impl FnMut(&AppState) -> bool) -> AppState {
        let mut snapshots = self.handle.subscribe();
        let wait = snapshots.wait_for(condition);
        let snapshot = tokio::time::timeout(WAIT_LIMIT, wait)
            .await
            .expect("the state did not reach the condition in time")
            .expect("the runner is running");
        snapshot.clone()
    }

    async fn wait_for_call(&self, call: &EngineCall) {
        tokio::time::timeout(WAIT_LIMIT, async {
            while !self.engine.calls().contains(call) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the engine did not get {call:?}"));
    }

    /// Sends `Shutdown` and waits until the runner has stopped.
    async fn shut_down(self) {
        self.handle.send(Message::Shutdown).await;
        tokio::time::timeout(WAIT_LIMIT, self.task)
            .await
            .expect("the runner did not stop in time")
            .expect("the runner did not panic");
        assert!(self.engine.calls().contains(&EngineCall::Shutdown));
    }
}

fn store() -> QueueStore {
    QueueStore::new(Database::open_in_memory().expect("database can be opened"))
}

fn queue_items(ids: &[&str]) -> Vec<QueueItem> {
    ids.iter()
        .map(|id| QueueItem {
            id: ItemId::new(*id),
            title: format!("title {id}"),
            artist: "artist".to_owned(),
            album: "album".to_owned(),
            duration: Duration::from_secs(180),
            artwork_template: None,
        })
        .collect()
}

fn ids(ids: &[&str]) -> Vec<ItemId> {
    ids.iter().map(|id| ItemId::new(*id)).collect()
}

fn restart_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/events/engine_restart.jsonl")
}

/// Returns the calls that matter for a restore, in order.
fn restore_calls(engine: &MockEngine) -> Vec<EngineCall> {
    engine
        .calls()
        .into_iter()
        .filter(|call| {
            matches!(
                call,
                EngineCall::SetQueue { .. } | EngineCall::Seek(_) | EngineCall::Play
            )
        })
        .collect()
}

#[tokio::test]
async fn queue_and_position_survive_an_application_restart() {
    let store = store();
    let first = Running::start(&store);
    first.engine.emit(EngineEvent::Ready);
    first
        .handle
        .send(Message::ReplaceQueue {
            items: queue_items(&["s1", "s2", "s3", "s4"]),
            start: 1,
        })
        .await;
    first.handle.send(Message::SetRepeat(RepeatMode::All)).await;
    first
        .handle
        .send(Message::SetShuffle {
            enabled: true,
            seed: 5,
        })
        .await;
    let before = first.wait_for(|state| state.queue.shuffle_enabled()).await;
    first.engine.emit(EngineEvent::Position {
        position: Duration::from_secs(61),
    });
    first
        .wait_for(|state| state.playback.position == Duration::from_secs(61))
        .await;
    first.shut_down().await;

    let second = Running::start(&store);
    let after = second
        .wait_for(|state| !state.queue.items().is_empty())
        .await;

    assert_eq!(after.queue, before.queue);
    assert_eq!(after.playback.position, Duration::from_secs(61));
    second.shut_down().await;
}

#[tokio::test]
async fn start_restore_sets_queue_and_seeks_without_playing() {
    let store = store();
    let mut queue = ringo_core::queue::Queue::new();
    let _ = queue.replace(queue_items(&["s1", "s2"]), 1);
    store
        .save(&queue, Duration::from_secs(30))
        .await
        .expect("save works");
    let running = Running::start(&store);

    running.engine.emit(EngineEvent::Ready);
    running
        .wait_for_call(&EngineCall::Seek(Duration::from_secs(30)))
        .await;

    assert_eq!(
        restore_calls(&running.engine),
        vec![
            EngineCall::SetQueue {
                items: ids(&["s1", "s2"]),
                start: 1
            },
            EngineCall::Seek(Duration::from_secs(30)),
        ]
    );
    running.shut_down().await;
}

#[tokio::test]
async fn engine_restart_restores_queue_position_and_playback() {
    let store = store();
    let running = Running::start(&store);
    running.engine.emit(EngineEvent::Ready);
    running
        .handle
        .send(Message::ReplaceQueue {
            items: queue_items(&["s1", "s2"]),
            start: 1,
        })
        .await;
    running
        .wait_for(|state| state.queue.items().len() == 2)
        .await;
    running.engine.emit(EngineEvent::State {
        state: PlaybackState::Playing,
    });
    running.engine.emit(EngineEvent::Position {
        position: Duration::from_secs(42),
    });
    running
        .wait_for(|state| state.playback.position == Duration::from_secs(42))
        .await;

    running.engine.emit_script(&restart_script());
    running
        .wait_for(|state| state.engine == EngineStatus::Ready)
        .await;
    running
        .wait_for_call(&EngineCall::Seek(Duration::from_secs(42)))
        .await;

    let set_queue = EngineCall::SetQueue {
        items: ids(&["s1", "s2"]),
        start: 1,
    };
    assert_eq!(
        restore_calls(&running.engine),
        vec![
            set_queue.clone(),
            EngineCall::Play,
            set_queue,
            EngineCall::Seek(Duration::from_secs(42)),
            EngineCall::Play,
        ]
    );
    running.shut_down().await;
}

/// Polls until the store has a queue that satisfies `condition`. The save runs on
/// a blocking thread, so the loop waits in real time.
async fn wait_for_saved(store: &QueueStore, condition: impl Fn(&Queue, Duration) -> bool) {
    let deadline = std::time::Instant::now() + WAIT_LIMIT;
    while std::time::Instant::now() < deadline {
        if let Some((queue, position)) = store.load().await.expect("load works")
            && condition(&queue, position)
        {
            return;
        }
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the store did not get the expected queue in time");
}

#[tokio::test(start_paused = true)]
async fn queue_change_is_saved_after_the_debounce_time() {
    let store = store();
    let running = Running::start(&store);
    running
        .handle
        .send(Message::ReplaceQueue {
            items: queue_items(&["s1", "s2"]),
            start: 0,
        })
        .await;
    running
        .handle
        .send(Message::Append(queue_items(&["s3"])))
        .await;

    tokio::time::sleep(Duration::from_millis(1100)).await;

    wait_for_saved(&store, |queue, _| queue.items().len() == 3).await;
    running.shut_down().await;
}

#[tokio::test(start_paused = true)]
async fn position_is_saved_every_ten_seconds_during_playback() {
    let store = store();
    let running = Running::start(&store);
    running.engine.emit(EngineEvent::Ready);
    running
        .handle
        .send(Message::ReplaceQueue {
            items: queue_items(&["s1"]),
            start: 0,
        })
        .await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    wait_for_saved(&store, |_, _| true).await;
    running.engine.emit(EngineEvent::State {
        state: PlaybackState::Playing,
    });
    running.engine.emit(EngineEvent::Position {
        position: Duration::from_secs(5),
    });
    running
        .wait_for(|state| state.playback.position == Duration::from_secs(5))
        .await;

    tokio::time::sleep(Duration::from_secs(10)).await;

    wait_for_saved(&store, |_, position| position == Duration::from_secs(5)).await;
    running.shut_down().await;
}

#[tokio::test]
async fn shutdown_during_an_engine_restart_saves_the_position_before_the_restart() {
    let store = store();
    let running = Running::start(&store);
    running.engine.emit(EngineEvent::Ready);
    running
        .handle
        .send(Message::ReplaceQueue {
            items: queue_items(&["s1"]),
            start: 0,
        })
        .await;
    running.engine.emit(EngineEvent::Position {
        position: Duration::from_secs(8),
    });
    running
        .wait_for(|state| state.playback.position == Duration::from_secs(8))
        .await;
    running.engine.emit(EngineEvent::Restarted);
    running
        .wait_for(|state| state.engine == EngineStatus::Restarting)
        .await;

    running.shut_down().await;

    let (_, position) = store.load().await.expect("load works").expect("saved");
    assert_eq!(position, Duration::from_secs(8));
}
