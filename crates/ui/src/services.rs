// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context;
use ringo_core::api_cache::{CachingTransport, SWEEP_INTERVAL};
use ringo_core::artwork::ArtworkStore;
use ringo_core::db::Database;
use ringo_core::paths::{PackageFormat, Paths};
use ringo_core::queue_store::QueueStore;
use ringo_core::runner::{Core, CoreHandle};
use ringo_core::settings::Settings;
use ringo_core::state::Message;
use ringo_engine::{EngineConfig, SidecarEngine, SidecarTransport};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tracing::warn;

/// Time that the application waits for the core to stop the engine.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// Bytes in one mebibyte. The cache limit setting is in mebibytes.
const MEBIBYTE: u64 = 1024 * 1024;

/// Access to the Apple Music API. The cache sits in front of the engine.
pub type ApiClient = CachingTransport<SidecarTransport>;

/// What the views need from the back end. Clones share the same services.
#[derive(Clone)]
pub struct AppServices {
    api: Arc<ApiClient>,
    core: CoreHandle,
    artwork: ArtworkStore,
    settings: Settings,
    paths: Arc<Paths>,
}

impl AppServices {
    /// The API transport with the response cache.
    pub fn api(&self) -> &Arc<ApiClient> {
        &self.api
    }

    /// The handle of the core.
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// The artwork files on disk.
    pub fn artwork(&self) -> &ArtworkStore {
        &self.artwork
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }
}

/// Owner of the running services (decision D12). It runs the core and the
/// engine on the runtime of relm4, and it stops them.
pub struct Services {
    shared: AppServices,
    runtime: Handle,
    runner: JoinHandle<()>,
    sweeper: JoinHandle<()>,
}

impl Services {
    /// Starts the engine, the cache, and the core. The core restores the queue
    /// of the last session.
    ///
    /// # Errors
    ///
    /// Returns an error if the directories, the database, or the engine cannot
    /// be set up.
    pub fn start(paths: Paths) -> anyhow::Result<Self> {
        let settings = Settings::new();
        let max_bytes = u64::from(settings.api_cache_max_mb()) * MEBIBYTE;
        let runtime = relm4_runtime();
        let (core, api, runner, sweeper) = runtime.block_on(async {
            paths
                .ensure()
                .context("cannot create the application directories")?;
            let database = Database::open(&paths.database).context("cannot open the database")?;
            let mut config = EngineConfig::from_env_or_default(
                paths.engine_data_dir.clone(),
                paths.runtime_dir.join("engine.sock"),
            )
            .context("cannot configure the engine")?;
            if PackageFormat::detect() == PackageFormat::Flatpak {
                config = config.through_zypak();
            }
            let engine = Arc::new(SidecarEngine::start(config).context("cannot start the engine")?);
            // The core subscribes to the engine events now. No `await` may come
            // between the engine start and this call, or `engine.ready` can be lost.
            let (core, handle) = Core::new(engine.clone());
            let core = core.with_queue_store(QueueStore::new(database.clone()));
            let runner = tokio::spawn(core.run());
            let api = CachingTransport::new(SidecarTransport::new(engine), database)
                .with_max_bytes(max_bytes);
            let sweeper = api.spawn_sweeper(SWEEP_INTERVAL);
            anyhow::Ok((handle, Arc::new(api), runner, sweeper))
        })?;
        let artwork = ArtworkStore::new(paths.artwork_cache_dir.clone())
            .context("cannot start the artwork cache")?;
        let pruned = artwork.clone();
        let artwork_max_bytes = u64::from(settings.artwork_cache_max_mb()) * MEBIBYTE;
        runtime.spawn_blocking(move || pruned.prune(artwork_max_bytes));
        let shared = AppServices {
            api,
            core,
            artwork,
            settings,
            paths: Arc::new(paths),
        };
        Ok(Self {
            shared,
            runtime,
            runner,
            sweeper,
        })
    }

    /// The services for the views.
    pub fn shared(&self) -> &AppServices {
        &self.shared
    }

    /// Stops the cache sweeper and the core. The core saves the queue and stops
    /// the engine. This call blocks until it is done, but not for longer than
    /// the shutdown timeout.
    pub fn shutdown(self) {
        let Self {
            shared,
            runtime,
            runner,
            sweeper,
        } = self;
        sweeper.abort();
        let stopped = runtime.block_on(async {
            shared.core.send(Message::Shutdown).await;
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
