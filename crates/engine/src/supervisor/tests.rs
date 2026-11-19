// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use tokio::task::{AbortHandle, JoinHandle};
use tokio::time;

use super::{ChildProcess, Launcher};
use crate::{EngineConfig, EngineEvent, Error, PlaybackEngine, SidecarEngine};

const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Directory for the socket of one test. Removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        Self(std::env::temp_dir().join(format!("ringo-{name}-{}", process::id())))
    }

    fn config(&self) -> EngineConfig {
        EngineConfig {
            command: PathBuf::from("unused"),
            args: Vec::new(),
            socket_path: self.0.join("ringomusic/engine.sock"),
            data_dir: self.0.join("data"),
        }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// How the fake engine behaves.
#[derive(Clone, Copy)]
enum Behavior {
    /// Connects, sends `engine.ready`, and answers requests.
    Serve,
    /// Never connects.
    Silent,
}

/// Launcher that runs a fake engine as a task in the test process.
#[derive(Clone)]
struct FakeLauncher {
    behavior: Behavior,
    launches: Arc<AtomicUsize>,
    tasks: Arc<Mutex<Vec<AbortHandle>>>,
}

impl FakeLauncher {
    fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            launches: Arc::default(),
            tasks: Arc::default(),
        }
    }

    fn launch_count(&self) -> usize {
        self.launches.load(Ordering::SeqCst)
    }

    /// Stops the newest fake engine, like a crash.
    fn crash_latest(&self) {
        let tasks = self.tasks.lock().expect("task list is not poisoned");
        tasks.last().expect("a fake engine was started").abort();
    }
}

impl Launcher for FakeLauncher {
    type Process = FakeProcess;

    fn launch(&self, config: &EngineConfig) -> io::Result<FakeProcess> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        let socket_path = config.socket_path.clone();
        let behavior = self.behavior;
        let task = tokio::spawn(async move {
            match behavior {
                Behavior::Serve => serve(&socket_path).await,
                Behavior::Silent => std::future::pending().await,
            }
        });
        self.tasks
            .lock()
            .expect("task list is not poisoned")
            .push(task.abort_handle());
        Ok(FakeProcess { task })
    }
}

struct FakeProcess {
    task: JoinHandle<()>,
}

impl ChildProcess for FakeProcess {
    async fn wait(&mut self) {
        let _ = (&mut self.task).await;
    }

    async fn kill(&mut self) {
        self.task.abort();
    }
}

/// The engine side of the protocol, in short form.
async fn serve(socket_path: &std::path::Path) {
    let stream = UnixStream::connect(socket_path)
        .await
        .expect("fake engine connects");
    let (read_half, mut writer) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();
    send(&mut writer, &hello()).await;
    send(
        &mut writer,
        &json!({"jsonrpc": "2.0", "method": "engine.ready"}),
    )
    .await;

    while let Ok(Some(line)) = lines.next_line().await {
        let request: Value = serde_json::from_str(&line).expect("client sends valid JSON");
        let result = if request["method"] == "auth.status" {
            json!({"authorized": true, "storefront": "us"})
        } else {
            Value::Null
        };
        send(
            &mut writer,
            &json!({"jsonrpc": "2.0", "id": request["id"], "result": result}),
        )
        .await;
        if request["method"] == "engine.shutdown" {
            return;
        }
    }
}

fn hello() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "engine.hello",
        "params": { "proto": 1, "engine_version": "0.1.0", "electron": "41.0.0" }
    })
}

async fn send(writer: &mut tokio::net::unix::OwnedWriteHalf, message: &Value) {
    let line = format!("{message}\n");
    writer
        .write_all(line.as_bytes())
        .await
        .expect("fake engine writes");
}

async fn next_event(events: &mut broadcast::Receiver<EngineEvent>) -> EngineEvent {
    time::timeout(EVENT_TIMEOUT, events.recv())
        .await
        .expect("an event arrives in time")
        .expect("the event channel stays open")
}

#[tokio::test]
async fn call_before_ready_returns_not_ready() {
    let dir = TestDir::new("not-ready");
    let engine = SidecarEngine::start_with(FakeLauncher::new(Behavior::Silent), dir.config())
        .expect("engine starts");

    assert!(matches!(engine.play().await, Err(Error::NotReady)));
    engine.shutdown().await.expect("shutdown works");
}

#[tokio::test]
async fn engine_restarts_after_it_exits() {
    let dir = TestDir::new("restart");
    let launcher = FakeLauncher::new(Behavior::Serve);
    let engine = SidecarEngine::start_with(launcher.clone(), dir.config()).expect("engine starts");
    let mut events = engine.events();

    assert_eq!(next_event(&mut events).await, EngineEvent::Ready);
    assert!(engine.auth_status().await.expect("status works").authorized);

    launcher.crash_latest();

    assert_eq!(next_event(&mut events).await, EngineEvent::Ready);
    assert_eq!(next_event(&mut events).await, EngineEvent::Restarted);
    assert_eq!(launcher.launch_count(), 2);
    assert!(engine.auth_status().await.expect("status works").authorized);
    engine.shutdown().await.expect("shutdown works");
}

#[tokio::test]
async fn shutdown_stops_the_engine_without_restart() {
    let dir = TestDir::new("shutdown");
    let launcher = FakeLauncher::new(Behavior::Serve);
    let engine = SidecarEngine::start_with(launcher.clone(), dir.config()).expect("engine starts");
    let mut events = engine.events();
    assert_eq!(next_event(&mut events).await, EngineEvent::Ready);

    engine.shutdown().await.expect("shutdown works");

    assert_eq!(launcher.launch_count(), 1);
    assert!(matches!(engine.play().await, Err(Error::NotReady)));
}
