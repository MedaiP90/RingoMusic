// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use ringo_engine::protocol::{AuthStatus, FromEngine, RepeatMode};
use ringo_engine::{EngineEvent, ItemId, PlaybackEngine, Result};
use tokio::sync::{broadcast, watch};

/// Capacity of the event channel.
const EVENT_CAPACITY: usize = 256;

/// A call that a test made on [`MockEngine`].
#[derive(Clone, Debug, PartialEq)]
pub enum EngineCall {
    /// `auth_status`.
    AuthStatus,
    /// `sign_in`.
    SignIn,
    /// `sign_out`.
    SignOut,
    /// `set_queue`.
    SetQueue {
        /// IDs in play order.
        items: Vec<ItemId>,
        /// Index of the first item.
        start: usize,
    },
    /// `play_next`.
    PlayNext(Vec<ItemId>),
    /// `play_later`.
    PlayLater(Vec<ItemId>),
    /// `remove`.
    Remove(usize),
    /// `play`.
    Play,
    /// `pause`.
    Pause,
    /// `stop`.
    Stop,
    /// `next`.
    Next,
    /// `previous`.
    Previous,
    /// `seek`.
    Seek(Duration),
    /// `set_volume`.
    SetVolume(f32),
    /// `set_repeat`.
    SetRepeat(RepeatMode),
    /// `restart`.
    Restart,
    /// `shutdown`.
    Shutdown,
}

/// A line of an event script: a notification of the engine, or an event that
/// the supervisor makes.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ScriptLine {
    Supervisor { supervisor: SupervisorEvent },
    Notification(FromEngine),
}

#[derive(Copy, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum SupervisorEvent {
    Restarted,
    GaveUp,
}

/// [`PlaybackEngine`] for tests. It records all calls and sends events that
/// the test gives. It does no playback.
#[derive(Debug)]
pub struct MockEngine {
    calls: Mutex<Vec<EngineCall>>,
    auth: Mutex<AuthStatus>,
    events: broadcast::Sender<EngineEvent>,
    lifecycle: watch::Sender<Option<EngineEvent>>,
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEngine {
    /// Creates an engine with no calls. The user is signed in with the
    /// storefront `us`.
    #[must_use]
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        Self {
            calls: Mutex::new(Vec::new()),
            auth: Mutex::new(AuthStatus {
                authorized: true,
                storefront: "us".to_owned(),
            }),
            events,
            lifecycle: watch::channel(None).0,
        }
    }

    /// Sets the answer of `auth_status`.
    pub fn set_auth_status(&self, status: AuthStatus) {
        *lock(&self.auth) = status;
    }

    /// Returns all calls in the order that they happened.
    #[must_use]
    pub fn calls(&self) -> Vec<EngineCall> {
        lock(&self.calls).clone()
    }

    /// Sends an event to all subscribers.
    pub fn emit(&self, event: EngineEvent) {
        if event.is_lifecycle() {
            self.lifecycle.send_replace(Some(event.clone()));
        }
        // No subscriber is not an error for a test double.
        let _ = self.events.send(event);
    }

    /// Sends the events of a JSON Lines script in order. A line is a
    /// notification of the engine (`{"method": …, "params": …}`) or a
    /// supervisor event (`{"supervisor": "restarted"}` or `"gave_up"`).
    /// Notifications that are not events, like `engine.hello`, are skipped.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be read or a line is not valid.
    pub fn emit_script(&self, path: &Path) {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read script {}: {error}", path.display()));
        for (number, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: ScriptLine = serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("{} line {}: {error}", path.display(), number + 1));
            let event = match parsed {
                ScriptLine::Supervisor {
                    supervisor: SupervisorEvent::Restarted,
                } => Some(EngineEvent::Restarted),
                ScriptLine::Supervisor {
                    supervisor: SupervisorEvent::GaveUp,
                } => Some(EngineEvent::GaveUp),
                ScriptLine::Notification(notification) => {
                    EngineEvent::from_notification(notification)
                }
            };
            if let Some(event) = event {
                self.emit(event);
            }
        }
    }

    fn record(&self, call: EngineCall) {
        lock(&self.calls).push(call);
    }
}

#[async_trait::async_trait]
impl PlaybackEngine for MockEngine {
    async fn auth_status(&self) -> Result<AuthStatus> {
        self.record(EngineCall::AuthStatus);
        Ok(lock(&self.auth).clone())
    }

    async fn sign_in(&self) -> Result<()> {
        self.record(EngineCall::SignIn);
        Ok(())
    }

    async fn sign_out(&self) -> Result<()> {
        self.record(EngineCall::SignOut);
        Ok(())
    }

    async fn set_queue(&self, items: Vec<ItemId>, start: usize) -> Result<()> {
        self.record(EngineCall::SetQueue { items, start });
        Ok(())
    }

    async fn play_next(&self, items: Vec<ItemId>) -> Result<()> {
        self.record(EngineCall::PlayNext(items));
        Ok(())
    }

    async fn play_later(&self, items: Vec<ItemId>) -> Result<()> {
        self.record(EngineCall::PlayLater(items));
        Ok(())
    }

    async fn remove(&self, index: usize) -> Result<()> {
        self.record(EngineCall::Remove(index));
        Ok(())
    }

    async fn play(&self) -> Result<()> {
        self.record(EngineCall::Play);
        Ok(())
    }

    async fn pause(&self) -> Result<()> {
        self.record(EngineCall::Pause);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.record(EngineCall::Stop);
        Ok(())
    }

    async fn next(&self) -> Result<()> {
        self.record(EngineCall::Next);
        Ok(())
    }

    async fn previous(&self) -> Result<()> {
        self.record(EngineCall::Previous);
        Ok(())
    }

    async fn seek(&self, position: Duration) -> Result<()> {
        self.record(EngineCall::Seek(position));
        Ok(())
    }

    async fn set_volume(&self, volume: f32) -> Result<()> {
        self.record(EngineCall::SetVolume(volume));
        Ok(())
    }

    async fn set_repeat(&self, mode: RepeatMode) -> Result<()> {
        self.record(EngineCall::SetRepeat(mode));
        Ok(())
    }

    async fn restart(&self) -> Result<()> {
        self.record(EngineCall::Restart);
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        self.record(EngineCall::Shutdown);
        Ok(())
    }

    fn events(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    fn lifecycle(&self) -> watch::Receiver<Option<EngineEvent>> {
        self.lifecycle.subscribe()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .expect("internal error: mock engine mutex is poisoned")
}
