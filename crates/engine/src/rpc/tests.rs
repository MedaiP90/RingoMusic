// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use super::RpcConnection;
use crate::protocol::{ErrorCode, FromEngine, PlaybackState, ToEngine};
use crate::{Error, Result};

const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// The engine side of the socket.
struct FakeEngine {
    lines: Lines<BufReader<OwnedReadHalf>>,
    socket: OwnedWriteHalf,
}

impl FakeEngine {
    fn pair() -> (UnixStream, Self) {
        let (client, server) = UnixStream::pair().expect("socket pair is created");
        let (read_half, socket) = server.into_split();
        let lines = BufReader::new(read_half).lines();
        (client, Self { lines, socket })
    }

    async fn send(&mut self, message: &Value) {
        let line = format!("{message}\n");
        self.socket
            .write_all(line.as_bytes())
            .await
            .expect("fake engine writes");
    }

    async fn send_hello(&mut self, proto: u32) {
        self.send(&hello(proto)).await;
    }

    async fn next_request(&mut self) -> Value {
        let line = self
            .lines
            .next_line()
            .await
            .expect("fake engine reads")
            .expect("client keeps the socket open");
        serde_json::from_str(&line).expect("client sends valid JSON")
    }
}

fn hello(proto: u32) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "engine.hello",
        "params": { "proto": proto, "engine_version": "0.1.0", "electron": "41.0.0" }
    })
}

async fn connect() -> (RpcConnection, FakeEngine) {
    let (client, mut engine) = FakeEngine::pair();
    engine.send_hello(1).await;
    let connection = RpcConnection::handshake(client)
        .await
        .expect("handshake succeeds");
    (connection, engine)
}

#[tokio::test]
async fn handshake_succeeds_for_matching_protocol() {
    let (_connection, _engine) = connect().await;
}

#[tokio::test]
async fn handshake_rejects_other_protocol_version() {
    let (client, mut engine) = FakeEngine::pair();
    engine.send_hello(2).await;

    let result = RpcConnection::handshake(client).await;

    assert!(matches!(
        result,
        Err(Error::ProtocolMismatch {
            expected: 1,
            actual: 2
        })
    ));
}

#[tokio::test]
async fn handshake_rejects_other_first_message() {
    let (client, mut engine) = FakeEngine::pair();
    engine
        .send(&json!({ "jsonrpc": "2.0", "method": "engine.ready" }))
        .await;

    let result = RpcConnection::handshake(client).await;

    assert!(matches!(result, Err(Error::InvalidMessage { .. })));
}

#[tokio::test(start_paused = true)]
async fn handshake_times_out_without_hello() {
    let (client, _engine) = FakeEngine::pair();

    let result = RpcConnection::handshake(client).await;

    assert!(matches!(result, Err(Error::HandshakeTimeout)));
}

#[tokio::test]
async fn concurrent_calls_get_their_own_answers() {
    let (connection, mut engine) = connect().await;

    let engine_task = async {
        let first = engine.next_request().await;
        let second = engine.next_request().await;
        // Answer in reverse order. The result names the method of the request.
        for request in [second, first] {
            engine
                .send(&json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": request["method"],
                }))
                .await;
        }
    };
    let (status, play, ()) = tokio::join!(
        connection.call(ToEngine::AuthStatus, CALL_TIMEOUT),
        connection.call(ToEngine::PlayerPlay, CALL_TIMEOUT),
        engine_task,
    );

    assert_eq!(status.expect("status answered"), json!("auth.status"));
    assert_eq!(play.expect("play answered"), json!("player.play"));
}

#[tokio::test]
async fn remote_error_becomes_error_remote() {
    let (connection, mut engine) = connect().await;

    let engine_task = async {
        let request = engine.next_request().await;
        engine
            .send(&json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": { "code": -32001, "message": "not signed in" },
            }))
            .await;
    };
    let (result, ()) = tokio::join!(
        connection.call(ToEngine::PlayerPlay, CALL_TIMEOUT),
        engine_task
    );

    match result {
        Err(Error::Remote { code, message }) => {
            assert_eq!(code, ErrorCode::NotAuthorized);
            assert_eq!(message, "not signed in");
        }
        other => panic!("expected a remote error, got {other:?}"),
    }
}

#[tokio::test]
async fn closed_stream_fails_pending_and_later_calls() {
    let (connection, mut engine) = connect().await;

    let engine_task = async {
        engine.next_request().await;
        drop(engine);
    };
    let (pending, ()) = tokio::join!(
        connection.call(ToEngine::PlayerPlay, CALL_TIMEOUT),
        engine_task
    );
    let later: Result<Value> = connection.call(ToEngine::PlayerPause, CALL_TIMEOUT).await;

    assert!(matches!(pending, Err(Error::ConnectionClosed)));
    assert!(matches!(later, Err(Error::ConnectionClosed)));
}

#[tokio::test(start_paused = true)]
async fn call_times_out_without_answer() {
    let (connection, _engine) = connect().await;

    let result = connection.call(ToEngine::PlayerPlay, CALL_TIMEOUT).await;

    assert!(matches!(result, Err(Error::RequestTimeout)));
}

#[tokio::test]
async fn notifications_reach_subscribers_and_invalid_lines_are_skipped() {
    let (connection, mut engine) = connect().await;
    let mut subscriber = connection.notifications();

    engine.socket.write_all(b"not json\n").await.expect("write");
    engine
        .send(&json!({
            "jsonrpc": "2.0",
            "method": "engine.log",
            "params": { "level": "info", "message": "hi", "fields": {} },
        }))
        .await;
    engine
        .send(&json!({
            "jsonrpc": "2.0",
            "method": "player.state",
            "params": { "state": "playing" },
        }))
        .await;

    let event = subscriber.recv().await.expect("notification arrives");
    assert_eq!(
        event,
        FromEngine::PlayerState {
            state: PlaybackState::Playing
        }
    );
}
