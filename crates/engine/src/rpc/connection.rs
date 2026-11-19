// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time;
use tracing::{debug, warn};

use super::incoming::Incoming;
use super::pending::PendingCalls;
use crate::protocol::{FromEngine, JsonRpcVersion, LogLevel, PROTOCOL_VERSION, Request, ToEngine};
use crate::{Error, Result};

/// Time that the engine has to send `engine.hello`.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// Notifications that a slow subscriber can lag behind before it loses some.
const NOTIFICATION_CAPACITY: usize = 256;
/// Lines that wait for the writer task.
const WRITE_QUEUE_CAPACITY: usize = 64;

type SocketLines = Lines<BufReader<OwnedReadHalf>>;

/// Connection to the engine after a successful handshake.
///
/// A reader task and a writer task do the socket I/O. Drop the connection to
/// close the socket. The engine quits when the socket closes.
#[derive(Debug)]
pub struct RpcConnection {
    next_id: AtomicU64,
    pending: PendingCalls,
    writer: mpsc::Sender<String>,
    notifications: broadcast::Sender<FromEngine>,
    reader_task: JoinHandle<()>,
}

impl RpcConnection {
    /// Waits for `engine.hello`, checks the protocol version, and starts the I/O tasks.
    ///
    /// # Errors
    ///
    /// Returns [`Error::HandshakeTimeout`] if the engine is silent for 30 seconds,
    /// [`Error::ProtocolMismatch`] if the engine uses another protocol version,
    /// [`Error::InvalidMessage`] if the first message is not `engine.hello`, and
    /// [`Error::ConnectionClosed`] or [`Error::Io`] if the socket fails.
    pub async fn handshake(stream: UnixStream) -> Result<Self> {
        let (read_half, write_half) = stream.into_split();
        let mut lines = BufReader::new(read_half).lines();
        time::timeout(HANDSHAKE_TIMEOUT, read_hello(&mut lines))
            .await
            .map_err(|_| Error::HandshakeTimeout)??;

        let pending = PendingCalls::new();
        let (notifications, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        let (writer, queue) = mpsc::channel(WRITE_QUEUE_CAPACITY);
        tokio::spawn(write_loop(write_half, queue));
        let reader_task = tokio::spawn(read_loop(lines, pending.clone(), notifications.clone()));

        Ok(Self {
            next_id: AtomicU64::new(1),
            pending,
            writer,
            notifications,
            reader_task,
        })
    }

    /// Sends a request and waits for its answer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Remote`] if the engine answers with an error,
    /// [`Error::RequestTimeout`] if there is no answer within `timeout`, and
    /// [`Error::ConnectionClosed`] if the socket is closed.
    pub async fn call(&self, request: ToEngine, timeout: Duration) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let line = encode(id, request)?;
        let answer = self.pending.register(id)?;

        if self.writer.send(line).await.is_err() {
            self.pending.cancel(id);
            return Err(Error::ConnectionClosed);
        }
        match time::timeout(timeout, answer).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => Err(Error::ConnectionClosed),
            Err(_) => {
                self.pending.cancel(id);
                Err(Error::RequestTimeout)
            }
        }
    }

    /// Subscribes to notifications of the engine.
    ///
    /// `engine.hello` and `engine.log` are not sent here. The handshake consumes
    /// the first one. The second one goes to `tracing`.
    #[must_use]
    pub fn notifications(&self) -> broadcast::Receiver<FromEngine> {
        self.notifications.subscribe()
    }
}

impl Drop for RpcConnection {
    fn drop(&mut self) {
        // The writer task ends when `writer` drops. The reader task needs a stop.
        self.reader_task.abort();
        self.pending.close();
    }
}

/// Reads the first message and checks that it is a `engine.hello` of this protocol version.
async fn read_hello(lines: &mut SocketLines) -> Result<()> {
    let line = lines.next_line().await?.ok_or(Error::ConnectionClosed)?;
    match Incoming::parse(&line)? {
        Incoming::Notification(FromEngine::EngineHello { proto, .. }) => {
            if proto == PROTOCOL_VERSION {
                Ok(())
            } else {
                Err(Error::ProtocolMismatch {
                    expected: PROTOCOL_VERSION,
                    actual: proto,
                })
            }
        }
        _ => Err(Error::InvalidMessage {
            detail: "the first message must be engine.hello".to_owned(),
        }),
    }
}

fn encode(id: u64, call: ToEngine) -> Result<String> {
    let request = Request {
        jsonrpc: JsonRpcVersion,
        id,
        call,
    };
    let mut line = serde_json::to_string(&request)?;
    line.push('\n');
    Ok(line)
}

/// Writes queued lines until the queue closes or a write fails.
async fn write_loop(mut socket: OwnedWriteHalf, mut queue: mpsc::Receiver<String>) {
    while let Some(line) = queue.recv().await {
        if let Err(error) = socket.write_all(line.as_bytes()).await {
            warn!(%error, "cannot write to the engine socket");
            break;
        }
    }
    // The peer can be gone already.
    let _ = socket.shutdown().await;
}

/// Routes each line until the socket closes, then fails all pending calls.
async fn read_loop(
    mut lines: SocketLines,
    pending: PendingCalls,
    notifications: broadcast::Sender<FromEngine>,
) {
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => route(&line, &pending, &notifications),
            Ok(None) => break,
            Err(error) => {
                warn!(%error, "cannot read from the engine socket");
                break;
            }
        }
    }
    pending.close();
}

fn route(line: &str, pending: &PendingCalls, notifications: &broadcast::Sender<FromEngine>) {
    match Incoming::parse(line) {
        Ok(Incoming::Notification(FromEngine::EngineLog {
            level,
            message,
            fields,
        })) => emit_log(level, &message, &fields),
        Ok(Incoming::Notification(event)) => {
            // No subscriber is a valid state.
            let _ = notifications.send(event);
        }
        Ok(Incoming::Response {
            id: Some(id),
            outcome,
        }) => {
            if !pending.complete(id, outcome) {
                debug!(id, "engine answered a request that nobody waits for");
            }
        }
        Ok(Incoming::Response { id: None, outcome }) => {
            warn!(?outcome, "engine could not read a request");
        }
        Err(error) => warn!(%error, "skipped an invalid line from the engine"),
    }
}

/// Re-emits an `engine.log` record with target `engine`.
fn emit_log(level: LogLevel, message: &str, fields: &serde_json::Map<String, Value>) {
    match level {
        LogLevel::Trace => tracing::trace!(target: "engine", ?fields, "{message}"),
        LogLevel::Debug => tracing::debug!(target: "engine", ?fields, "{message}"),
        LogLevel::Info => tracing::info!(target: "engine", ?fields, "{message}"),
        LogLevel::Warn => tracing::warn!(target: "engine", ?fields, "{message}"),
        LogLevel::Error => tracing::error!(target: "engine", ?fields, "{message}"),
    }
}
