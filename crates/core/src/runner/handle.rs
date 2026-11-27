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

    /// Returns a sender for the inbox. Use it for code that sends messages from
    /// a thread without a runtime. The runner does not stop while a sender
    /// exists, so stop it with [`Message::Shutdown`].
    #[must_use]
    pub fn sender(&self) -> mpsc::Sender<Message> {
        self.inbox.clone()
    }

    /// Returns the storefront of the signed-in user, from the last snapshot.
    /// `None` when nobody is signed in.
    #[must_use]
    pub fn storefront(&self) -> Option<String> {
        self.snapshots.borrow().storefront().map(str::to_owned)
    }

    /// Returns a receiver of state snapshots.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<AppState> {
        self.snapshots.clone()
    }
}
