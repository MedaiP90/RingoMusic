// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Checks that engine calls run in a worker and that the core does not wait
//! for them. The clock is paused, so a slow call costs no real time.

use std::sync::Arc;
use std::time::Duration;

use ringo_core::runner::{Core, CoreHandle};
use ringo_core::state::{AppState, AuthState, Message};
use ringo_core::{EngineCall, MockEngine};
use ringo_engine::EngineEvent;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Duration of a slow engine call.
const SLOW_CALL: Duration = Duration::from_secs(25);

struct Fixture {
    engine: Arc<MockEngine>,
    handle: CoreHandle,
    runner: JoinHandle<()>,
}

impl Fixture {
    /// Starts a runner on a mock engine that is ready and signed in.
    async fn start() -> Self {
        let engine = Arc::new(MockEngine::new());
        let (core, handle) = Core::new(Arc::clone(&engine) as _);
        let runner = tokio::spawn(core.run());
        let fixture = Self {
            engine,
            handle,
            runner,
        };
        fixture.engine.emit(EngineEvent::Ready);
        fixture
            .wait_for(|state| matches!(state.auth, AuthState::SignedIn { .. }))
            .await;
        fixture
    }

    async fn wait_for(&self, condition: impl FnMut(&AppState) -> bool) {
        self.handle
            .subscribe()
            .wait_for(condition)
            .await
            .expect("the runner is running");
    }
}

#[tokio::test(start_paused = true)]
async fn slow_engine_call_does_not_delay_a_ui_message() {
    let fixture = Fixture::start().await;
    fixture.engine.set_delay(SLOW_CALL);
    fixture.handle.send(Message::Play).await;
    let start = Instant::now();

    fixture.handle.send(Message::SetVolume(0.5)).await;
    fixture
        .wait_for(|state| (state.playback.volume - 0.5).abs() < f32::EPSILON)
        .await;

    assert!(
        start.elapsed() < Duration::from_secs(1),
        "the core waited for the engine"
    );
}

#[tokio::test(start_paused = true)]
async fn engine_calls_arrive_in_the_order_of_the_commands() {
    let fixture = Fixture::start().await;
    fixture.engine.set_delay(Duration::from_millis(10));
    let before = fixture.engine.calls().len();

    fixture.handle.send(Message::SetVolume(0.2)).await;
    fixture.handle.send(Message::Pause).await;
    fixture.handle.send(Message::Play).await;
    fixture.handle.send(Message::Stop).await;
    fixture
        .handle
        .send(Message::Seek(Duration::from_secs(5)))
        .await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(
        fixture.engine.calls()[before..],
        [
            EngineCall::SetVolume(0.2),
            EngineCall::Pause,
            EngineCall::Play,
            EngineCall::Stop,
            EngineCall::Seek(Duration::from_secs(5)),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn shutdown_stops_the_engine_after_the_queued_calls() {
    let fixture = Fixture::start().await;
    fixture.engine.set_delay(SLOW_CALL);

    fixture.handle.send(Message::Pause).await;
    fixture.handle.send(Message::Shutdown).await;
    fixture.runner.await.expect("the runner does not panic");

    let calls = fixture.engine.calls();
    assert_eq!(calls.last(), Some(&EngineCall::Shutdown));
    assert!(calls.contains(&EngineCall::Pause));
}
