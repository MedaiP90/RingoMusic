// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;
use std::time::Duration;

use ringo_engine::protocol::{ErrorCode, PlaybackState};
use ringo_engine::{EngineEvent, Error, ItemId, PlaybackEngine, Result};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{self, Instant, MissedTickBehavior};
use tracing::{debug, warn};

use super::CoreHandle;
use super::engine_queue::{Edit, EngineQueue};
use crate::queue_store::QueueStore;
use crate::state::{AppState, Command, Message, update};

/// Capacity of the message inbox.
const INBOX_CAPACITY: usize = 64;

/// Time to wait after a queue change before the queue is saved. Many changes
/// in this time make one save.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

/// Time between two saves of the position during playback.
const POSITION_SAVE_INTERVAL: Duration = Duration::from_secs(10);

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
            engine_queue: EngineQueue::default(),
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
        self.restore_queue().await;
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
            self.handle(message).await;
        }
        self.shutdown().await;
    }

    /// Saves the queue, then stops the engine.
    pub async fn shutdown(&mut self) {
        self.save_due = None;
        self.save_queue().await;
        if let Err(error) = self.engine.shutdown().await {
            warn!(%error, "cannot stop the engine");
        }
    }

    async fn handle(&mut self, message: Message) {
        let mut message = message;
        if changes_queue(&message) {
            self.save_due = Some(Instant::now() + SAVE_DEBOUNCE);
        }
        self.observe(&mut message);
        let commands = update(&mut self.state, message);
        self.snapshots.send_replace(self.state.clone());
        for command in commands {
            if let Err(error) = self.execute(&command).await {
                warn!(%error, ?command, "engine command failed");
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
                self.handle(Message::RestoreQueue { queue, position }).await;
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
        // While the engine restarts, the live position is 0. The waiting resume has the real one.
        let position = state
            .resume
            .map_or(state.playback.position, |resume| resume.position);
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
            Command::RestartEngine => self.engine.restart().await,
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
            let failed = sign_in && !run_sign_in(engine.as_ref()).await;
            match engine.auth_status().await {
                Ok(status) => {
                    // A closed inbox means that the runner stopped.
                    let _ = inbox.send(Message::AuthLoaded(status)).await;
                }
                Err(error) => warn!(%error, "cannot read the authorization"),
            }
            if sign_in {
                // The authorization comes first, so the user sees no flash of the sign-in page.
                let _ = inbox.send(Message::SignInEnded { failed }).await;
            }
        });
    }
}

/// Shows the login window. Returns `false` if the login failed. A cancel by
/// the user is not a failure.
async fn run_sign_in(engine: &dyn PlaybackEngine) -> bool {
    match engine.sign_in().await {
        Ok(()) => true,
        Err(Error::Remote {
            code: ErrorCode::Cancelled,
            ..
        }) => {
            debug!("the user cancelled the sign in");
            true
        }
        Err(error) => {
            warn!(%error, "sign in failed");
            false
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
            | Message::Append(_)
            | Message::PlayNext(_)
            | Message::Remove(_)
            | Message::Move { .. }
            | Message::SetShuffle { .. }
            | Message::SetRepeat(_)
            | Message::Engine(EngineEvent::NowPlaying { .. })
    )
}
