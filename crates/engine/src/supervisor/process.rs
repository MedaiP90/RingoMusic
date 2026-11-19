// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::future::Future;
use std::io;
use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, warn};

use crate::EngineConfig;

/// Environment variable with the socket path.
const SOCKET_ENV: &str = "RINGO_ENGINE_SOCKET";
/// Environment variable with the data directory.
const DATA_DIR_ENV: &str = "RINGO_ENGINE_DATA_DIR";

/// Starts engine processes. Tests replace it with an in-process fake.
pub(crate) trait Launcher: Send + Sync + 'static {
    type Process: ChildProcess;

    /// Starts one engine process.
    fn launch(&self, config: &EngineConfig) -> io::Result<Self::Process>;
}

/// One running engine process.
pub(crate) trait ChildProcess: Send + 'static {
    /// Resolves when the process exits.
    fn wait(&mut self) -> impl Future<Output = ()> + Send;

    /// Stops the process. Does nothing if it already exited.
    fn kill(&mut self) -> impl Future<Output = ()> + Send;
}

/// Starts the real engine as a child process.
#[derive(Debug)]
pub(crate) struct SystemLauncher;

impl Launcher for SystemLauncher {
    type Process = SystemProcess;

    fn launch(&self, config: &EngineConfig) -> io::Result<SystemProcess> {
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .env(SOCKET_ENV, &config.socket_path)
            .env(DATA_DIR_ENV, &config.data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            // Own process group: Ctrl+C in a terminal must not kill the engine
            // before the graceful shutdown ran.
            .process_group(0)
            .spawn()?;
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(forward_output("stdout", stdout));
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(forward_output("stderr", stderr));
        }
        Ok(SystemProcess { child })
    }
}

/// A real child process.
#[derive(Debug)]
pub(crate) struct SystemProcess {
    child: Child,
}

impl ChildProcess for SystemProcess {
    async fn wait(&mut self) {
        match self.child.wait().await {
            Ok(status) => log_exit(status),
            Err(error) => warn!(%error, "cannot wait for the engine process"),
        }
    }

    async fn kill(&mut self) {
        if let Err(error) = self.child.kill().await {
            debug!(%error, "cannot kill the engine process");
        }
    }
}

fn log_exit(status: ExitStatus) {
    debug!(%status, "engine process exited");
}

/// Sends each output line of the process to `tracing` at level `debug`.
async fn forward_output<R: AsyncRead + Unpin>(stream: &'static str, reader: R) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        debug!(target: "engine::process", stream, "{line}");
    }
}
