// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::future::Future;
use std::io;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio::time;
use tracing::{debug, warn};

use crate::EngineConfig;

/// Environment variable with the socket path.
const SOCKET_ENV: &str = "RINGO_ENGINE_SOCKET";
/// Environment variable with the data directory.
const DATA_DIR_ENV: &str = "RINGO_ENGINE_DATA_DIR";
/// Parts of the Chromium messages that report a sandbox failure.
const SANDBOX_MARKERS: [&str; 2] = ["chrome-sandbox", "No usable sandbox"];
/// Time that the supervisor waits for the last output lines after an exit.
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

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

    /// Returns `true` if the output of the process reported a sandbox failure.
    /// The answer is complete after [`ChildProcess::wait`] returns.
    fn sandbox_failed(&self) -> bool;
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
        let sandbox_failed = Arc::new(AtomicBool::new(false));
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(tokio::spawn(forward_output(
                "stdout",
                stdout,
                Arc::clone(&sandbox_failed),
            )));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(tokio::spawn(forward_output(
                "stderr",
                stderr,
                Arc::clone(&sandbox_failed),
            )));
        }
        Ok(SystemProcess {
            child,
            readers,
            sandbox_failed,
        })
    }
}

/// A real child process.
#[derive(Debug)]
pub(crate) struct SystemProcess {
    child: Child,
    /// Tasks that read the output. They end when the pipes close.
    readers: Vec<JoinHandle<()>>,
    sandbox_failed: Arc<AtomicBool>,
}

impl ChildProcess for SystemProcess {
    async fn wait(&mut self) {
        match self.child.wait().await {
            Ok(status) => log_exit(status),
            Err(error) => warn!(%error, "cannot wait for the engine process"),
        }
        // Read the last lines, so `sandbox_failed` is complete. A helper process
        // can keep a pipe open, so the wait has a limit.
        let drain = async {
            for reader in self.readers.drain(..) {
                let _ = reader.await;
            }
        };
        let _ = time::timeout(OUTPUT_DRAIN_TIMEOUT, drain).await;
    }

    async fn kill(&mut self) {
        if let Err(error) = self.child.kill().await {
            debug!(%error, "cannot kill the engine process");
        }
    }

    fn sandbox_failed(&self) -> bool {
        self.sandbox_failed.load(Ordering::SeqCst)
    }
}

fn log_exit(status: ExitStatus) {
    debug!(%status, "engine process exited");
}

/// Sends each output line of the process to `tracing` at level `debug`. Sets
/// `sandbox_failed` when a line reports a sandbox failure.
async fn forward_output<R: AsyncRead + Unpin>(
    stream: &'static str,
    reader: R,
    sandbox_failed: Arc<AtomicBool>,
) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if SANDBOX_MARKERS.iter().any(|marker| line.contains(marker)) {
            sandbox_failed.store(true, Ordering::SeqCst);
        }
        debug!(target: "engine::process", stream, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_shell(script: &str) -> SystemProcess {
        let config = EngineConfig {
            command: "sh".into(),
            args: vec!["-c".into(), script.into()],
            socket_path: "unused".into(),
            data_dir: "unused".into(),
        };
        SystemLauncher.launch(&config).expect("sh starts")
    }

    #[tokio::test]
    async fn sandbox_failed_is_true_after_exit_with_sandbox_message() {
        let mut process = run_shell("echo 'FATAL: No usable sandbox!' >&2; exit 1");
        process.wait().await;
        assert!(process.sandbox_failed());
    }

    #[tokio::test]
    async fn sandbox_failed_is_false_after_exit_without_sandbox_message() {
        let mut process = run_shell("echo 'other error' >&2; exit 1");
        process.wait().await;
        assert!(!process.sandbox_failed());
    }
}
