// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::Value;
use tokio::sync::oneshot;

use crate::{Error, Result};

type Answer = oneshot::Sender<Result<Value>>;

/// Requests that wait for an answer, by request ID.
///
/// The map is `None` after the connection closed. Then no new request can wait.
#[derive(Clone, Debug, Default)]
pub(super) struct PendingCalls {
    calls: Arc<Mutex<Option<HashMap<u64, Answer>>>>,
}

impl PendingCalls {
    pub(super) fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Some(HashMap::new()))),
        }
    }

    /// Starts to wait for the answer of request `id`.
    pub(super) fn register(&self, id: u64) -> Result<oneshot::Receiver<Result<Value>>> {
        let (sender, receiver) = oneshot::channel();
        self.lock()
            .as_mut()
            .ok_or(Error::ConnectionClosed)?
            .insert(id, sender);
        Ok(receiver)
    }

    /// Gives the answer to the waiting request. Returns `false` if nobody waits.
    pub(super) fn complete(&self, id: u64, outcome: Result<Value>) -> bool {
        let Some(sender) = self.remove(id) else {
            return false;
        };
        // The caller can stop to wait at any time, so a failed send is normal.
        let _ = sender.send(outcome);
        true
    }

    /// Stops to wait for request `id`.
    pub(super) fn cancel(&self, id: u64) {
        self.remove(id);
    }

    /// Fails all waiting requests and refuses new ones.
    pub(super) fn close(&self) {
        let calls = self.lock().take().unwrap_or_default();
        for sender in calls.into_values() {
            let _ = sender.send(Err(Error::ConnectionClosed));
        }
    }

    fn remove(&self, id: u64) -> Option<Answer> {
        self.lock().as_mut()?.remove(&id)
    }

    fn lock(&self) -> MutexGuard<'_, Option<HashMap<u64, Answer>>> {
        self.calls
            .lock()
            .expect("internal error: pending calls mutex is poisoned")
    }
}
