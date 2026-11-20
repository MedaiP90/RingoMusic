// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use tokio::sync::{mpsc, watch};

use crate::state::{AppState, Message};

/// Handle for the users of [`Core`](super::Core): it sends messages and reads
/// snapshots. When all handles are dropped, the runner stops.
#[derive(Clone, Debug)]
pub struct CoreHandle {
    pub(super) inbox: mpsc::Sender<Message>,
    pub(super) snapshots: watch::Receiver<AppState>,
}

impl CoreHandle {
    /// Sends a message to the runner. The message is dropped if the runner
    /// stopped.
    pub async fn send(&self, message: Message) {
        // A closed inbox means that the runner stopped, so nobody can use the message.
        let _ = self.inbox.send(message).await;
    }

    /// Returns a receiver of state snapshots.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<AppState> {
        self.snapshots.clone()
    }
}
