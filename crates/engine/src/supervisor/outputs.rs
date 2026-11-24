// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;

use tokio::sync::{broadcast, watch};

use crate::{EngineEvent, RpcConnection};

/// What the supervisor publishes: the live connection and the events.
#[derive(Debug)]
pub(crate) struct Outputs {
    /// Connection to a ready engine, or `None` while the engine is down.
    pub(crate) connection: watch::Sender<Option<Arc<RpcConnection>>>,
    pub(crate) events: broadcast::Sender<EngineEvent>,
    /// Last lifecycle event, or `None` before the first one.
    pub(crate) lifecycle: watch::Sender<Option<EngineEvent>>,
}

impl Outputs {
    /// Sends an event. A lifecycle event is also kept for late subscribers.
    /// Having no subscriber is a valid state.
    pub(super) fn emit(&self, event: EngineEvent) {
        if event.is_lifecycle() {
            self.lifecycle.send_replace(Some(event.clone()));
        }
        let _ = self.events.send(event);
    }
}
