// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;

use ringo_engine::{EngineEvent, ItemId, PlaybackEngine, Result};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc, watch};
use tracing::warn;

use super::CoreHandle;
use super::engine_queue::{Edit, EngineQueue};
use crate::state::{AppState, Command, Message, update};

/// Capacity of the message inbox.
const INBOX_CAPACITY: usize = 64;

/// Owner of the application state. It runs [`update`] for each message, calls
/// the engine for each command, and publishes a state snapshot.
pub struct Core {
    state: AppState,
    engine: Arc<dyn PlaybackEngine>,
    inbox: mpsc::Receiver<Message>,
    /// Lets background tasks send messages without a hold on the inbox.
    feedback: mpsc::WeakSender<Message>,
    events: broadcast::Receiver<EngineEvent>,
    snapshots: watch::Sender<AppState>,
    engine_queue: EngineQueue,
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
            engine_queue: EngineQueue::default(),
        };
        let handle = CoreHandle {
            inbox: sender,
            snapshots: snapshots_receiver,
        };
        (core, handle)
    }

    /// Handles messages and engine events until all handles are dropped or the
    /// engine closes its event channel.
    pub async fn run(mut self) {
        loop {
            let message = tokio::select! {
                message = self.inbox.recv() => match message {
                    Some(message) => message,
                    None => break,
                },
                event = self.events.recv() => match event {
                    Ok(event) => Message::Engine(event),
                    Err(RecvError::Lagged(skipped)) => {
                        warn!(skipped, "the core lost engine events");
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                },
            };
            self.handle(message).await;
        }
    }

    async fn handle(&mut self, message: Message) {
        let mut message = message;
        self.observe(&mut message);
        let commands = update(&mut self.state, message);
        self.snapshots.send_replace(self.state.clone());
        for command in commands {
            if let Err(error) = self.execute(&command).await {
                warn!(%error, ?command, "engine command failed");
            }
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
                self.engine_queue.set_current(*queue_index);
                *queue_index = self.rust_index(id, *queue_index);
            }
            EngineEvent::Restarted => self.engine_queue.set(Vec::new(), 0),
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
        if self.engine_queue.is_aligned() {
            return engine_index;
        }
        let mapped = self.engine_queue.to_rust_index(engine_index);
        let queue = &self.state.queue;
        let known = queue.playback_order().get(mapped) == Some(id);
        if known {
            mapped
        } else {
            queue.current_playback_index().unwrap_or(mapped)
        }
    }

    async fn execute(&mut self, command: &Command) -> Result<()> {
        match command {
            Command::SetQueue { items, start } => {
                self.engine_queue.set(items.clone(), *start);
                self.engine.set_queue(items.clone(), *start).await
            }
            Command::SyncUpcoming { items, current } => self.sync_upcoming(items, *current).await,
            Command::Play => self.engine.play().await,
            Command::Pause => self.engine.pause().await,
            Command::Stop => self.engine.stop().await,
            Command::Next => self.engine.next().await,
            Command::PlayItem { id } => self.play_item(id).await,
            Command::Seek(position) => self.engine.seek(*position).await,
            Command::SetVolume(volume) => self.engine.set_volume(*volume).await,
            Command::SetRepeat(mode) => self.engine.set_repeat(*mode).await,
            Command::RefreshAuth => {
                self.spawn_auth_task(false);
                Ok(())
            }
            Command::SignIn => {
                self.spawn_auth_task(true);
                Ok(())
            }
            Command::SignOut => {
                self.engine.sign_out().await?;
                self.spawn_auth_task(false);
                Ok(())
            }
        }
    }

    /// Plays an item at once: inserts it after the current item and skips to
    /// it. This works for any item, whatever the engine has before the
    /// current item.
    async fn play_item(&mut self, id: &ItemId) -> Result<()> {
        let items = vec![id.clone()];
        self.engine.play_next(items.clone()).await?;
        self.engine_queue.apply(&Edit::PlayNext(items));
        self.engine.next().await?;
        self.engine_queue.step_forward();
        Ok(())
    }

    /// Changes the upcoming items of the engine queue with small edits, so
    /// the current item does not stop.
    async fn sync_upcoming(&mut self, items: &[ItemId], current: usize) -> Result<()> {
        let Some(edits) = self.engine_queue.plan(items, current) else {
            // The current item of the engine is not the current item of the queue.
            warn!("the engine plays another item than the queue, so the edit was skipped");
            return Ok(());
        };
        for edit in edits {
            match &edit {
                Edit::Remove(index) => self.engine.remove(*index).await?,
                Edit::PlayNext(items) => self.engine.play_next(items.clone()).await?,
                Edit::PlayLater(items) => self.engine.play_later(items.clone()).await?,
            }
            self.engine_queue.apply(&edit);
        }
        self.engine_queue.align(current);
        Ok(())
    }

    /// Reads the authorization in the background and sends it to the inbox.
    /// The login has no time limit, so it must not block the runner.
    fn spawn_auth_task(&self, sign_in: bool) {
        let Some(inbox) = self.feedback.upgrade() else {
            return;
        };
        let engine = Arc::clone(&self.engine);
        tokio::spawn(async move {
            if sign_in && let Err(error) = engine.sign_in().await {
                warn!(%error, "sign in failed");
            }
            match engine.auth_status().await {
                Ok(status) => {
                    // A closed inbox means that the runner stopped.
                    let _ = inbox.send(Message::AuthLoaded(status)).await;
                }
                Err(error) => warn!(%error, "cannot read the authorization"),
            }
        });
    }
}
