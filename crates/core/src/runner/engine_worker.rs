// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::HashSet;
use std::mem;
use std::sync::Arc;

use ringo_engine::protocol::ErrorCode;
use ringo_engine::{Error, ItemId, PlaybackEngine, Result};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use super::engine_queue::{Edit, SharedEngineQueue};
use crate::state::{Command, Message};

/// Task that runs the engine calls, one after the other, in the order that
/// the core sent them. A slow call does not stop the core. The engine itself
/// limits the time of each call.
pub(super) struct EngineWorker {
    commands: mpsc::UnboundedSender<Command>,
    task: JoinHandle<()>,
}

impl EngineWorker {
    /// Starts the worker. `feedback` lets it send messages to the core.
    pub(super) fn spawn(
        engine: Arc<dyn PlaybackEngine>,
        queue: SharedEngineQueue,
        feedback: mpsc::WeakSender<Message>,
    ) -> Self {
        let (commands, inbox) = mpsc::unbounded_channel();
        let runner = Runner {
            engine,
            queue,
            feedback,
        };
        let task = tokio::spawn(runner.run(inbox));
        Self { commands, task }
    }

    /// Queues a command. This does not wait for the engine.
    pub(super) fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            warn!("the engine worker stopped, so a command was lost");
        }
    }

    /// Runs the queued commands, then stops the engine. Returns when the
    /// engine is stopped.
    pub(super) async fn finish(self) {
        let Self { commands, task } = self;
        drop(commands);
        if let Err(error) = task.await {
            warn!(%error, "the engine worker panicked");
        }
    }
}

struct Runner {
    engine: Arc<dyn PlaybackEngine>,
    queue: SharedEngineQueue,
    feedback: mpsc::WeakSender<Message>,
}

impl Runner {
    /// Runs commands until the core drops its sender. The engine stops last.
    async fn run(self, mut inbox: mpsc::UnboundedReceiver<Command>) {
        while let Some(first) = inbox.recv().await {
            let mut waiting = vec![first];
            while let Ok(command) = inbox.try_recv() {
                waiting.push(command);
            }
            for command in drop_replaced(waiting) {
                debug!(?command, "sending an engine command");
                let logged = command.clone();
                if let Err(error) = self.execute(command).await {
                    warn!(%error, command = ?logged, "engine command failed");
                }
            }
        }
        if let Err(error) = self.engine.shutdown().await {
            warn!(%error, "cannot stop the engine");
        }
    }

    async fn execute(&self, command: Command) -> Result<()> {
        match command {
            Command::SetQueue { items, start } => {
                self.queue.lock().set(items.clone(), start);
                self.engine.set_queue(items, start).await
            }
            Command::SyncUpcoming { items, current } => self.sync_upcoming(&items, current).await,
            Command::Play => self.engine.play().await,
            Command::Pause => self.engine.pause().await,
            Command::Stop => self.engine.stop().await,
            Command::Next => self.engine.next().await,
            Command::PlayItem { id } => self.play_item(id).await,
            Command::Seek(position) => self.engine.seek(position).await,
            Command::SetVolume(volume) => self.engine.set_volume(volume).await,
            Command::SetRepeat(mode) => self.engine.set_repeat(mode).await,
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
    async fn play_item(&self, id: ItemId) -> Result<()> {
        let items = vec![id];
        self.engine.play_next(items.clone()).await?;
        self.queue.lock().apply(&Edit::PlayNext(items));
        self.engine.next().await?;
        self.queue.lock().step_forward();
        Ok(())
    }

    /// Changes the upcoming items of the engine queue with small edits, so
    /// the current item does not stop.
    async fn sync_upcoming(&self, items: &[ItemId], current: usize) -> Result<()> {
        let plan = self.queue.lock().plan(items, current);
        let Some(edits) = plan else {
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
            self.queue.lock().apply(&edit);
        }
        self.queue.lock().align(current);
        Ok(())
    }

    /// Reads the authorization in the background and sends it to the core.
    /// The login has no time limit, so it must not block the worker.
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

/// Removes each `SetVolume` and `Seek` that a later command of the same type
/// replaces. A slider drag during a slow call then gives one engine call, not
/// one call for each step.
fn drop_replaced(commands: Vec<Command>) -> Vec<Command> {
    let mut seen = HashSet::new();
    let mut kept: Vec<Command> = commands
        .into_iter()
        .rev()
        .filter(|command| {
            let replaceable = matches!(command, Command::SetVolume(_) | Command::Seek(_));
            !replaceable || seen.insert(mem::discriminant(command))
        })
        .collect();
    kept.reverse();
    kept
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn drop_replaced_keeps_last_volume_and_seek_and_all_other_commands() {
        let commands = vec![
            Command::SetVolume(0.1),
            Command::Seek(Duration::from_secs(1)),
            Command::Pause,
            Command::SetVolume(0.2),
            Command::Seek(Duration::from_secs(2)),
            Command::Play,
            Command::SetVolume(0.3),
        ];
        assert_eq!(
            drop_replaced(commands),
            vec![
                Command::Pause,
                Command::Seek(Duration::from_secs(2)),
                Command::Play,
                Command::SetVolume(0.3),
            ]
        );
    }
}
