// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;
use std::time::Instant;

use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::time;
use tracing::{error, info, warn};

use super::outputs::Outputs;
use super::process::Launcher;
use super::restart_policy::{RestartWindow, backoff_delay};
use super::session::{self, SessionEnd, StopSignal};
use crate::{EngineConfig, EngineEvent};

/// Runs the engine process and restarts it after a failure.
///
/// The supervisor keeps no queue state. After a restart it only emits
/// [`EngineEvent::Restarted`].
pub(crate) struct Supervisor<L: Launcher> {
    pub(crate) launcher: L,
    pub(crate) config: EngineConfig,
    pub(crate) listener: UnixListener,
    pub(crate) outputs: Outputs,
    pub(crate) stop: StopSignal,
    /// Requests to start the engine again after the supervisor gave up.
    pub(crate) restart: mpsc::Receiver<()>,
}

impl<L: Launcher> Supervisor<L> {
    /// Runs until the stop signal fires. If the engine fails too often, it
    /// emits [`EngineEvent::GaveUp`] and waits for a restart request.
    pub(crate) async fn run(mut self) {
        let mut window = RestartWindow::new();
        let mut attempt: u32 = 0;
        let mut was_ready = false;
        loop {
            let end = self.run_session(was_ready).await;
            match end {
                SessionEnd::Stopped => return,
                SessionEnd::SandboxFailed => {
                    // A restart cannot fix the sandbox. Wait for the user.
                    if !self.wait_for_restart_request().await {
                        return;
                    }
                    window = RestartWindow::new();
                    attempt = 0;
                    continue;
                }
                SessionEnd::Failed { reached_ready } if reached_ready => {
                    was_ready = true;
                    attempt = 0;
                }
                SessionEnd::Failed { .. } => {}
            }

            if !window.try_record(Instant::now()) {
                error!("engine failed too often, the supervisor gives up");
                self.outputs.emit(EngineEvent::GaveUp);
                if !self.wait_for_restart_request().await {
                    return;
                }
                window = RestartWindow::new();
                attempt = 0;
                continue;
            }
            let delay = backoff_delay(attempt);
            attempt = attempt.saturating_add(1);
            if self.wait_before_restart(delay).await {
                return;
            }
        }
    }

    async fn run_session(&mut self, announce_restart: bool) -> SessionEnd {
        match self.launcher.launch(&self.config) {
            Ok(process) => {
                session::run(
                    process,
                    &self.listener,
                    &self.outputs,
                    &mut self.stop,
                    announce_restart,
                )
                .await
            }
            Err(error) => {
                warn!(
                    %error,
                    command = %self.config.command.display(),
                    "cannot start the engine process"
                );
                SessionEnd::Failed {
                    reached_ready: false,
                }
            }
        }
    }

    /// Waits after a give-up or a sandbox failure. Returns `true` on a restart request, and `false`
    /// if the stop signal fired.
    async fn wait_for_restart_request(&mut self) -> bool {
        // A request from before the give-up is old.
        while self.restart.try_recv().is_ok() {}
        tokio::select! {
            Some(()) = self.restart.recv() => true,
            _ = &mut self.stop => false,
        }
    }

    /// Sleeps for `delay`. Returns `true` if the stop signal fired meanwhile.
    async fn wait_before_restart(&mut self, delay: Duration) -> bool {
        info!(?delay, "restarting the engine after a delay");
        tokio::select! {
            () = time::sleep(delay) => false,
            _ = &mut self.stop => true,
        }
    }
}
