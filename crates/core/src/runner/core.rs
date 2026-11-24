// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;
use std::time::Duration;

use ringo_engine::protocol::PlaybackState;
use ringo_engine::{EngineEvent, ItemId, PlaybackEngine};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{self, Instant, MissedTickBehavior};
use tracing::warn;

use super::CoreHandle;
use super::engine_queue::SharedEngineQueue;
use super::engine_worker::EngineWorker;
use crate::queue_store::QueueStore;
use crate::state::{AppState, Message, update};

/// Capacity of the message inbox.
const INBOX_CAPACITY: usize = 64;

/// Time to wait after a queue change before the queue is saved. Many changes
/// in this time make one save.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

/// Time between two saves of the position during playback.
const POSITION_SAVE_INTERVAL: Duration = Duration::from_secs(10);

/// Owner of the application state. It runs [`update`] for each message, calls
/// sends each command to the engine worker, and publishes a state snapshot.
pub struct Core {
    state: AppState,
    engine: Arc<dyn PlaybackEngine>,
    inbox: mpsc::Receiver<Message>,
    /// Lets background tasks send messages without a hold on the inbox.
    feedback: mpsc::WeakSender<Message>,
    events: broadcast::Receiver<EngineEvent>,
    snapshots: watch::Sender<AppState>,
    engine_queue: SharedEngineQueue,
    /// Runs the engine calls. It exists while [`Core::run`] runs.
    worker: Option<EngineWorker>,
    store: Option<QueueStore>,
    /// When the changed queue must be saved. `None` if nothing is waiting.
    save_due: Option<Instant>,
}

impl Core {
    /// Creates the runner and the handle for its users. The runner subscribes
    /// to the events of the engine now. Call [`Core::run`] to start it.
    #[must_use]
    pub fn new(engine: Arc<dyn PlaybackEngine>) -> (Self, CoreHandle) {
        let state = AppState::new();
        let (sender, inbox) = mpsc::channel(INBOX_CAPACITY);
        let (snapshots, snapshots_receiver) = watch::channel(state.clone());
        let core = Self {
            state,
            events: engine.events(),
            engine,
            inbox,
            feedback: sender.downgrade(),
            snapshots,
            engine_queue: SharedEngineQueue::default(),
            worker: None,
            store: None,
            save_due: None,
        };
        let handle = CoreHandle {
            inbox: sender,
            snapshots: snapshots_receiver,
        };
        (core, handle)
    }

    /// Saves the queue in `store` and restores it when the runner starts.
    #[must_use]
    pub fn with_queue_store(mut self, store: QueueStore) -> Self {
        self.store = Some(store);
        self
    }

    /// Handles messages and engine events until a [`Message::Shutdown`], until
    /// all handles are dropped, or until the engine closes its event channel.
    /// Then it calls [`Core::shutdown`].
    pub async fn run(mut self) {
        self.worker = Some(EngineWorker::spawn(
            Arc::clone(&self.engine),
            self.engine_queue.clone(),
            self.feedback.clone(),
        ));
        self.restore_queue().await;
        self.apply_current_lifecycle();
        let mut position_timer = time::interval_at(
            Instant::now() + POSITION_SAVE_INTERVAL,
            POSITION_SAVE_INTERVAL,
        );
        position_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            let save_due = self.save_due;
            let message = tokio::select! {
                message = self.inbox.recv() => match message {
                    Some(Message::Shutdown) | None => break,
                    Some(message) => message,
                },
                () = time::sleep_until(save_due.unwrap_or_else(Instant::now)), if save_due.is_some() => {
                    self.save_due = None;
                    self.save_queue().await;
                    continue;
                }
                _ = position_timer.tick() => {
                    self.save_position().await;
                    continue;
                }
                event = self.events.recv() => match event {
                    Ok(event) => Message::Engine(event),
                    Err(RecvError::Lagged(skipped)) => {
                        warn!(skipped, "the core lost engine events");
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                },
            };
            self.handle(message);
        }
        self.shutdown().await;
    }

    /// Applies the last lifecycle event of the engine, which can come before
    /// the runner started. The runner already listens to the events, so the same
    /// event can come twice. The state change is the same for both.
    fn apply_current_lifecycle(&mut self) {
        let current = self.engine.lifecycle().borrow().clone();
        if let Some(event) = current {
            self.handle(Message::Engine(event));
        }
    }

    /// Saves the queue, then stops the engine after the queued engine calls.
    pub async fn shutdown(&mut self) {
        self.save_due = None;
        self.save_queue().await;
        if let Some(worker) = self.worker.take() {
            worker.finish().await;
        } else if let Err(error) = self.engine.shutdown().await {
            warn!(%error, "cannot stop the engine");
        }
    }

    fn handle(&mut self, message: Message) {
        let mut message = message;
        if changes_queue(&message) {
            self.save_due = Some(Instant::now() + SAVE_DEBOUNCE);
        }
        self.observe(&mut message);
        let commands = update(&mut self.state, message);
        self.snapshots.send_replace(self.state.clone());
        if let Some(worker) = &self.worker {
            for command in commands {
                worker.send(command);
            }
        }
    }

    /// Loads the saved queue and puts it in the state. A failure is logged and
    /// the application starts with an empty queue.
    async fn restore_queue(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        match store.load().await {
            Ok(Some((queue, position))) => {
                self.handle(Message::RestoreQueue { queue, position });
            }
            Ok(None) => {}
            Err(error) => warn!(%error, "cannot load the saved queue"),
        }
    }

    async fn save_queue(&self) {
        let Some(store) = &self.store else {
            return;
        };
        let state = &self.state;
        // While the engine restarts, the live position is 0. The waiting resume
        // or the waiting seek has the real one.
        let position = state
            .resume
            .map(|resume| resume.position)
            .or(state.pending_seek)
            .unwrap_or(state.playback.position);
        if let Err(error) = store.save(&state.queue, position).await {
            warn!(%error, "cannot save the queue");
        }
    }

    /// Saves the position if an item plays.
    async fn save_position(&self) {
        let Some(store) = &self.store else {
            return;
        };
        if self.state.playback.state != PlaybackState::Playing {
            return;
        }
        if let Err(error) = store.save_position(self.state.playback.position).await {
            warn!(%error, "cannot save the position");
        }
    }

    /// Does the parts of a message that are not state: logs and the copy of
    /// the engine queue. It also changes the queue index of `NowPlaying` from
    /// the engine index to the Rust index.
    fn observe(&mut self, message: &mut Message) {
        let Message::Engine(event) = message else {
            return;
        };
        match event {
            EngineEvent::NowPlaying {
                id, queue_index, ..
            } => {
                self.engine_queue.lock().set_current(*queue_index);
                *queue_index = self.rust_index(id, *queue_index);
            }
            EngineEvent::Restarted => self.engine_queue.lock().set(Vec::new(), 0),
            EngineEvent::Error {
                code,
                message,
                fatal,
            } => warn!(?code, %message, fatal, "the engine reported an error"),
            _ => {}
        }
    }

    /// Returns the index in the Rust queue of the item that the engine plays.
    /// If the engine plays an item that the Rust queue does not have there
    /// (an old item before the current item), the current index stays.
    fn rust_index(&self, id: &ItemId, engine_index: usize) -> usize {
        let mapped = {
            let engine_queue = self.engine_queue.lock();
            if engine_queue.is_aligned() {
                return engine_index;
            }
            engine_queue.to_rust_index(engine_index)
        };
        let queue = &self.state.queue;
        let known = queue.playback_order().get(mapped) == Some(id);
        if known {
            mapped
        } else {
            queue.current_playback_index().unwrap_or(mapped)
        }
    }
}

/// Returns `true` if the message can change the queue, so the queue needs a save.
fn changes_queue(message: &Message) -> bool {
    matches!(
        message,
        Message::Next
            | Message::Previous
            | Message::ReplaceQueue { .. }
            | Message::ReplaceQueueShuffled { .. }
            | Message::Append(_)
            | Message::PlayNext(_)
            | Message::Remove { .. }
            | Message::Move { .. }
            | Message::SetShuffle { .. }
            | Message::SetRepeat(_)
            | Message::Engine(EngineEvent::NowPlaying { .. })
    )
}
