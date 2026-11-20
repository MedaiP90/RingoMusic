// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use rand::SeedableRng;
use rand::rngs::StdRng;
use ringo_engine::EngineEvent;
use ringo_engine::protocol::PlaybackState;

use super::{AppState, AuthState, Command, EngineStatus, Message, NowPlaying};
use crate::queue::{Queue, QueueChange};

/// Applies `message` to `state` and returns the engine calls that follow.
///
/// This function does no I/O. It gives the same result for the same input.
pub fn update(state: &mut AppState, message: Message) -> Vec<Command> {
    match message {
        Message::Play => vec![Command::Play],
        Message::Pause => vec![Command::Pause],
        Message::Next => skip_forward(&mut state.queue),
        Message::Previous => skip_back(&mut state.queue),
        Message::Seek(position) => vec![Command::Seek(position)],
        Message::SetVolume(volume) => {
            let volume = volume.clamp(0.0, 1.0);
            state.playback.volume = volume;
            vec![Command::SetVolume(volume)]
        }
        Message::SetRepeat(mode) => {
            state.queue.set_repeat(mode);
            vec![Command::SetRepeat(mode)]
        }
        Message::SignIn => vec![Command::SignIn],
        Message::SignOut => vec![Command::SignOut],
        Message::AuthLoaded(status) => {
            state.auth = if status.authorized {
                AuthState::SignedIn {
                    storefront: status.storefront,
                }
            } else {
                AuthState::SignedOut
            };
            Vec::new()
        }
        Message::Engine(event) => apply_event(state, event),
        message => edit_queue(state, message),
    }
}

/// Handles the messages that change the queue.
fn edit_queue(state: &mut AppState, message: Message) -> Vec<Command> {
    let was_playing = state.playback.state == PlaybackState::Playing;
    let queue = &mut state.queue;
    let (change, autoplay) = match message {
        Message::ReplaceQueue { items, start } => (queue.replace(items, start), true),
        Message::Append(items) => (queue.append(items), was_playing),
        Message::PlayNext(items) => (queue.play_next(items), was_playing),
        Message::Remove(index) => (queue.remove(index), was_playing),
        Message::Move { from, to } => (queue.move_item(from, to), was_playing),
        Message::SetShuffle { enabled, seed } => {
            let mut rng = StdRng::seed_from_u64(seed);
            (queue.set_shuffle(enabled, &mut rng), was_playing)
        }
        _ => return Vec::new(),
    };
    queue_commands(&state.queue, change, autoplay)
}

/// Converts a queue change to engine calls. `autoplay` starts playback after
/// the engine gets a new queue.
fn queue_commands(queue: &Queue, change: QueueChange, autoplay: bool) -> Vec<Command> {
    let Some(current) = queue.current_playback_index() else {
        return match change {
            QueueChange::Replaced => vec![Command::Stop],
            QueueChange::None | QueueChange::UpcomingOnly => Vec::new(),
        };
    };
    let items = queue.playback_order();
    match change {
        QueueChange::None => Vec::new(),
        QueueChange::UpcomingOnly => vec![Command::SyncUpcoming { items, current }],
        QueueChange::Replaced => {
            let mut commands = vec![Command::SetQueue {
                items,
                start: current,
            }];
            if autoplay {
                commands.push(Command::Play);
            }
            commands
        }
    }
}

/// The user skips forward. Repeat one restarts the item. At the end of the
/// queue, nothing happens.
fn skip_forward(queue: &mut Queue) -> Vec<Command> {
    let before = queue.current_playback_index();
    match queue.advance() {
        None => Vec::new(),
        Some(position) if Some(position) == before => vec![Command::Seek(Duration::ZERO)],
        Some(_) => vec![Command::Next],
    }
}

/// The user skips back. At the start of the queue, the item restarts.
fn skip_back(queue: &mut Queue) -> Vec<Command> {
    let Some(before) = queue.current_playback_index() else {
        return Vec::new();
    };
    match queue.go_back() {
        Some(position) if position != before => play_current(queue),
        Some(_) | None => vec![Command::Seek(Duration::ZERO)],
    }
}

/// The engine must play the current item of the queue now. The engine
/// queue before the current item can differ from the queue, so the engine
/// gets the item itself, and then the upcoming items.
fn play_current(queue: &Queue) -> Vec<Command> {
    let (Some(item), Some(current)) = (queue.current(), queue.current_playback_index()) else {
        return Vec::new();
    };
    vec![
        Command::PlayItem {
            id: item.id.clone(),
        },
        Command::SyncUpcoming {
            items: queue.playback_order(),
            current,
        },
    ]
}

fn apply_event(state: &mut AppState, event: EngineEvent) -> Vec<Command> {
    match event {
        EngineEvent::Ready => on_ready(state),
        EngineEvent::Incompatible { check, detail } => {
            state.engine = EngineStatus::Incompatible { check, detail };
            Vec::new()
        }
        EngineEvent::AuthChanged { authorized: true } => vec![Command::RefreshAuth],
        EngineEvent::AuthChanged { authorized: false } => {
            state.auth = AuthState::SignedOut;
            Vec::new()
        }
        EngineEvent::State { state: playback } => {
            state.playback.state = playback;
            Vec::new()
        }
        EngineEvent::NowPlaying {
            id,
            title,
            artist,
            album,
            duration,
            artwork_url,
            queue_index,
        } => {
            // An index outside the queue is ignored. The queue stays as it is.
            let _ = state.queue.sync_to_playback_index(queue_index);
            state.playback.position = Duration::ZERO;
            state.now_playing = Some(NowPlaying {
                id,
                title,
                artist,
                album,
                duration,
                artwork_url,
            });
            Vec::new()
        }
        EngineEvent::Position { position } => {
            state.playback.position = position;
            Vec::new()
        }
        EngineEvent::Restarted => {
            state.engine = EngineStatus::Restarting;
            state.playback.state = PlaybackState::Stopped;
            state.playback.position = Duration::ZERO;
            Vec::new()
        }
        EngineEvent::GaveUp => {
            state.engine = EngineStatus::GaveUp;
            Vec::new()
        }
        // Errors are logged by the runner. New events are not known yet.
        _ => Vec::new(),
    }
}

/// The engine is ready. After a restart, the new engine has no queue, so
/// the queue goes to it again. Playback stays stopped.
fn on_ready(state: &mut AppState) -> Vec<Command> {
    let restarted = state.engine == EngineStatus::Restarting;
    state.engine = EngineStatus::Ready;
    let mut commands = vec![
        Command::RefreshAuth,
        Command::SetVolume(state.playback.volume),
        Command::SetRepeat(state.queue.repeat()),
    ];
    if restarted && let Some(start) = state.queue.current_playback_index() {
        commands.push(Command::SetQueue {
            items: state.queue.playback_order(),
            start,
        });
    }
    commands
}
