// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{broadcast, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::warn;

use crate::protocol::{AuthStatus, QueueKind, RepeatMode, ToEngine};
use crate::supervisor::{self, Launcher, Outputs, Supervisor, SystemLauncher};
use crate::{EngineConfig, EngineEvent, Error, ItemId, PlaybackEngine, Result, RpcConnection};

/// Time limit for a request. `auth.signIn` does not use it.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Time limit that is long enough to be no limit.
const NO_TIMEOUT: Duration = Duration::from_secs(365 * 24 * 60 * 60);
/// Capacity of the event channel.
const EVENT_CAPACITY: usize = 256;

/// [`PlaybackEngine`] that runs the Electron engine as a supervised process.
#[derive(Debug)]
pub struct SidecarEngine {
    connection: watch::Receiver<Option<Arc<RpcConnection>>>,
    events: broadcast::Sender<EngineEvent>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SidecarEngine {
    /// Binds the socket and starts the supervisor, which starts the engine process.
    ///
    /// Returns before the engine is ready. Wait for [`EngineEvent::Ready`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the socket directory or the socket cannot be created.
    ///
    /// # Panics
    ///
    /// Panics if no Tokio runtime is running.
    pub fn start(config: EngineConfig) -> Result<Self> {
        Self::start_with(SystemLauncher, config)
    }

    /// Does the same as [`SidecarEngine::start`] with another process launcher.
    pub(crate) fn start_with<L: Launcher>(launcher: L, config: EngineConfig) -> Result<Self> {
        let listener = supervisor::bind(&config.socket_path)?;
        let (connection_sender, connection) = watch::channel(None);
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (stop_sender, stop) = oneshot::channel();
        let supervisor = Supervisor {
            launcher,
            config,
            listener,
            outputs: Outputs {
                connection: connection_sender,
                events: events.clone(),
            },
            stop,
        };
        let task = tokio::spawn(supervisor.run());
        Ok(Self {
            connection,
            events,
            stop: Mutex::new(Some(stop_sender)),
            task: Mutex::new(Some(task)),
        })
    }

    /// Runs a GET request on the Apple Music API. Returns the raw JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready, does not answer, or the API call fails.
    pub async fn api_get(&self, path: &str, query: BTreeMap<String, String>) -> Result<Value> {
        let request = ToEngine::ApiGet {
            path: path.to_owned(),
            query,
        };
        self.call(request, CALL_TIMEOUT).await
    }

    async fn call(&self, request: ToEngine, timeout: Duration) -> Result<Value> {
        let connection = self.connection.borrow().clone().ok_or(Error::NotReady)?;
        connection.call(request, timeout).await
    }

    /// Sends a request whose result is not used.
    async fn command(&self, request: ToEngine) -> Result<()> {
        self.call(request, CALL_TIMEOUT).await.map(drop)
    }
}

#[async_trait::async_trait]
impl PlaybackEngine for SidecarEngine {
    async fn auth_status(&self) -> Result<AuthStatus> {
        let value = self.call(ToEngine::AuthStatus, CALL_TIMEOUT).await?;
        Ok(serde_json::from_value(value)?)
    }

    async fn sign_in(&self) -> Result<()> {
        self.call(ToEngine::AuthSignIn, NO_TIMEOUT).await.map(drop)
    }

    async fn sign_out(&self) -> Result<()> {
        self.command(ToEngine::AuthSignOut).await
    }

    async fn set_queue(&self, items: Vec<ItemId>, start: usize) -> Result<()> {
        let start_index = u32::try_from(start).map_err(|_| Error::InvalidMessage {
            detail: format!("queue start index {start} is too large"),
        })?;
        self.command(ToEngine::PlayerSetQueue {
            items: items.into_iter().map(ItemId::into_string).collect(),
            start_index,
            kind: QueueKind::Songs,
        })
        .await
    }

    async fn play(&self) -> Result<()> {
        self.command(ToEngine::PlayerPlay).await
    }

    async fn pause(&self) -> Result<()> {
        self.command(ToEngine::PlayerPause).await
    }

    async fn stop(&self) -> Result<()> {
        self.command(ToEngine::PlayerStop).await
    }

    async fn next(&self) -> Result<()> {
        self.command(ToEngine::PlayerNext).await
    }

    async fn previous(&self) -> Result<()> {
        self.command(ToEngine::PlayerPrevious).await
    }

    async fn seek(&self, position: Duration) -> Result<()> {
        let position_ms = u64::try_from(position.as_millis()).unwrap_or(u64::MAX);
        self.command(ToEngine::PlayerSeek { position_ms }).await
    }

    async fn set_volume(&self, volume: f32) -> Result<()> {
        self.command(ToEngine::PlayerSetVolume { volume }).await
    }

    async fn set_repeat(&self, mode: RepeatMode) -> Result<()> {
        self.command(ToEngine::PlayerSetRepeat { mode }).await
    }

    async fn shutdown(&self) -> Result<()> {
        if let Some(stop) = lock(&self.stop).take() {
            // The supervisor can be gone already.
            let _ = stop.send(());
        }
        let task = lock(&self.task).take();
        if let Some(task) = task
            && let Err(error) = task.await
        {
            warn!(%error, "the engine supervisor task failed");
        }
        Ok(())
    }

    fn events(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .expect("internal error: sidecar engine mutex is poisoned")
}
