// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::Arc;
use std::time::Duration;

use tokio::net::UnixListener;
use tokio::sync::{broadcast, oneshot};
use tokio::time;
use tracing::{debug, error, warn};

use super::outputs::Outputs;
use super::process::ChildProcess;
use crate::protocol::{ErrorCode, FromEngine, ToEngine};
use crate::{EngineEvent, Error, Result, RpcConnection};

/// Time that the engine has to connect to the socket.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Time that the engine has to send `engine.ready` after the handshake.
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Time that the engine has to quit after `engine.shutdown`.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Signal to stop. It fires when the sender sends or drops.
pub(super) type StopSignal = oneshot::Receiver<()>;

/// How a session of one engine process ended.
#[derive(Debug)]
pub(super) enum SessionEnd {
    /// The caller asked to stop. Do not restart.
    Stopped,
    /// A restart cannot help, for example when the sandbox cannot start, the
    /// page failed the self-checks, or the protocol versions differ. Wait for the user. The reason is already emitted.
    NeedsUser,
    /// The engine failed. Restart it if the policy allows.
    Failed {
        /// The engine was ready before it failed.
        reached_ready: bool,
    },
}

/// Result of the wait for the first engine message after the handshake.
enum Startup {
    Ready(RpcConnection, broadcast::Receiver<FromEngine>),
    Incompatible { check: String, detail: String },
}

/// Runs one engine process from connection to exit, and stops the process at the end.
pub(super) async fn run<P: ChildProcess>(
    mut process: P,
    listener: &UnixListener,
    outputs: &Outputs,
    stop: &mut StopSignal,
    announce_restart: bool,
) -> SessionEnd {
    let end = drive(&mut process, listener, outputs, stop, announce_restart).await;
    outputs.connection.send_replace(None);
    process.kill().await;
    end
}

async fn drive<P: ChildProcess>(
    process: &mut P,
    listener: &UnixListener,
    outputs: &Outputs,
    stop: &mut StopSignal,
    announce_restart: bool,
) -> SessionEnd {
    let startup = tokio::select! {
        result = start_up(listener, outputs) => result,
        () = process.wait() => {
            if process.sandbox_failed() {
                return report_sandbox_failure(outputs);
            }
            warn!("engine process exited before it was ready");
            return SessionEnd::Failed { reached_ready: false };
        }
        _ = &mut *stop => return SessionEnd::Stopped,
    };
    let (connection, mut events) = match startup {
        Ok(Startup::Ready(connection, events)) => (Arc::new(connection), events),
        Ok(Startup::Incompatible { check, detail }) => {
            warn!(%check, %detail, "engine reports that it is incompatible");
            outputs.emit(EngineEvent::Incompatible { check, detail });
            return SessionEnd::NeedsUser;
        }
        Err(Error::ProtocolMismatch { expected, actual }) => {
            warn!(expected, actual, "engine uses another protocol version");
            outputs.emit(EngineEvent::ProtocolMismatch { expected, actual });
            return SessionEnd::NeedsUser;
        }
        Err(error) => {
            warn!(%error, "engine did not become ready");
            return SessionEnd::Failed {
                reached_ready: false,
            };
        }
    };

    outputs
        .connection
        .send_replace(Some(Arc::clone(&connection)));
    // `Restarted` comes first: the core keeps the resume point, and `Ready` uses it.
    if announce_restart {
        outputs.emit(EngineEvent::Restarted);
    }
    outputs.emit(EngineEvent::Ready);
    forward_events(process, &connection, &mut events, outputs, stop).await
}

fn report_sandbox_failure(outputs: &Outputs) -> SessionEnd {
    error!("the Chromium sandbox of the engine cannot start");
    outputs.emit(EngineEvent::Error {
        code: ErrorCode::SandboxFailed,
        message: "the Chromium sandbox cannot start".to_owned(),
        fatal: true,
    });
    SessionEnd::NeedsUser
}

/// Forwards events until the engine fails or the caller stops it.
async fn forward_events<P: ChildProcess>(
    process: &mut P,
    connection: &RpcConnection,
    events: &mut broadcast::Receiver<FromEngine>,
    outputs: &Outputs,
    stop: &mut StopSignal,
) -> SessionEnd {
    loop {
        tokio::select! {
            received = events.recv() => match received {
                Ok(notification) => emit_notification(outputs, notification),
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    warn!(count, "the supervisor lost engine notifications");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            () = connection.closed() => break,
            () = process.wait() => break,
            _ = &mut *stop => {
                stop_gracefully(connection, process).await;
                return SessionEnd::Stopped;
            }
        }
    }
    warn!("engine connection ended");
    SessionEnd::Failed {
        reached_ready: true,
    }
}

/// Accepts the connection, does the handshake, and waits for `engine.ready`.
async fn start_up(listener: &UnixListener, outputs: &Outputs) -> Result<Startup> {
    let (stream, _address) = time::timeout(CONNECT_TIMEOUT, listener.accept())
        .await
        .map_err(|_| Error::HandshakeTimeout)??;
    let (connection, mut events) = RpcConnection::handshake_with_events(stream).await?;
    let first = time::timeout(READY_TIMEOUT, wait_for_first_verdict(&mut events, outputs))
        .await
        .map_err(|_| Error::ReadyTimeout)??;
    Ok(match first {
        Verdict::Ready => Startup::Ready(connection, events),
        Verdict::Incompatible { check, detail } => Startup::Incompatible { check, detail },
    })
}

enum Verdict {
    Ready,
    Incompatible { check: String, detail: String },
}

/// Waits for `engine.ready` or `engine.incompatible`. Forwards other events.
async fn wait_for_first_verdict(
    events: &mut broadcast::Receiver<FromEngine>,
    outputs: &Outputs,
) -> Result<Verdict> {
    loop {
        match events.recv().await {
            Ok(FromEngine::EngineReady) => return Ok(Verdict::Ready),
            Ok(FromEngine::EngineIncompatible { check, detail }) => {
                return Ok(Verdict::Incompatible { check, detail });
            }
            Ok(other) => emit_notification(outputs, other),
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return Err(Error::ConnectionClosed),
        }
    }
}

fn emit_notification(outputs: &Outputs, notification: FromEngine) {
    if let Some(event) = EngineEvent::from_notification(notification) {
        outputs.emit(event);
    }
}

/// Sends `engine.shutdown` and waits for the process to quit. The caller kills it after.
async fn stop_gracefully<P: ChildProcess>(connection: &RpcConnection, process: &mut P) {
    let quit = async {
        if let Err(error) = connection
            .call(ToEngine::EngineShutdown, SHUTDOWN_TIMEOUT)
            .await
        {
            debug!(%error, "engine.shutdown failed");
        }
        process.wait().await;
    };
    if time::timeout(SHUTDOWN_TIMEOUT, quit).await.is_err() {
        warn!("engine did not quit in time and will be killed");
    }
}
