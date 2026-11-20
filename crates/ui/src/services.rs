// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context;
use ringo_core::db::Database;
use ringo_core::paths::Paths;
use ringo_core::queue_store::QueueStore;
use ringo_core::runner::{Core, CoreHandle};
use ringo_core::state::Message;
use ringo_engine::{EngineConfig, SidecarEngine};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tracing::warn;

/// Time that the application waits for the core to stop the engine.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// The core and the engine, running on the runtime of relm4 (decision D12).
pub struct Services {
    runtime: Handle,
    core: CoreHandle,
    runner: JoinHandle<()>,
}

impl Services {
    /// Starts the engine and the core. The queue of the last session is
    /// restored.
    ///
    /// # Errors
    ///
    /// Returns an error if the paths, the database, or the engine cannot be set up.
    pub fn start() -> anyhow::Result<Self> {
        let runtime = relm4_runtime();
        let (core, runner) = runtime.block_on(async {
            let paths = Paths::from_env().context("cannot resolve the application paths")?;
            paths
                .ensure()
                .context("cannot create the application directories")?;
            let database = Database::open(&paths.database).context("cannot open the database")?;
            let config = EngineConfig::from_env_or_default(
                paths.engine_data_dir,
                paths.runtime_dir.join("engine.sock"),
            )
            .context("cannot configure the engine")?;
            let engine = Arc::new(SidecarEngine::start(config).context("cannot start the engine")?);
            let (core, handle) = Core::new(engine);
            let core = core.with_queue_store(QueueStore::new(database));
            anyhow::Ok((handle, tokio::spawn(core.run())))
        })?;
        Ok(Self {
            runtime,
            core,
            runner,
        })
    }

    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Stops the core. The core saves the queue and stops the engine. This call
    /// blocks until it is done, but not for longer than the shutdown timeout.
    pub fn shutdown(self) {
        let Self {
            runtime,
            core,
            runner,
        } = self;
        let stopped = runtime.block_on(async {
            core.send(Message::Shutdown).await;
            tokio::time::timeout(SHUTDOWN_TIMEOUT, runner).await
        });
        if stopped.is_err() {
            warn!("the core did not stop in time");
        }
    }
}

/// Returns a handle to the Tokio runtime of relm4. The runtime is created at
/// the first use, and it lives until the process ends.
fn relm4_runtime() -> Handle {
    let (sender, receiver) = mpsc::channel();
    relm4::spawn(async move {
        // The receiver waits, so the send cannot fail.
        let _ = sender.send(Handle::current());
    });
    receiver
        .recv()
        .expect("internal error: the relm4 runtime did not run the task")
}
