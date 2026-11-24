// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use ringo_engine::protocol::{AuthStatus, ErrorCode, PlaybackState, RepeatMode};
use ringo_engine::{EngineEvent, ItemId};

use super::*;
use crate::queue::QueueItem;

fn item(id: &str) -> QueueItem {
    QueueItem {
        id: ItemId::new(id),
        title: format!("title {id}"),
        artist: "artist".to_owned(),
        album: "album".to_owned(),
        duration: Duration::from_secs(180),
        artwork_template: None,
    }
}

fn items(ids: &[&str]) -> Vec<QueueItem> {
    ids.iter().map(|id| item(id)).collect()
}

fn ids(ids: &[&str]) -> Vec<ItemId> {
    ids.iter().map(|id| ItemId::new(*id)).collect()
}

/// State with a queue of `names` that starts at `start`.
fn state_with(names: &[&str], start: usize) -> AppState {
    let mut state = AppState::new();
    let _ = update(
        &mut state,
        Message::ReplaceQueue {
            items: items(names),
            start,
        },
    );
    state
}

fn current_id(state: &AppState) -> &str {
    state
        .queue
        .current()
        .expect("queue has a current item")
        .id
        .as_str()
}

fn event(state: &mut AppState, event: EngineEvent) -> Vec<Command> {
    update(state, Message::Engine(event))
}

#[test]
fn replace_queue_shuffled_sends_one_queue_and_plays() {
    let mut state = AppState::new();
    let commands = update(
        &mut state,
        Message::ReplaceQueueShuffled {
            items: items(&["a", "b", "c"]),
            seed: 7,
        },
    );
    assert!(state.queue.shuffle_enabled());
    assert!(matches!(
        commands.as_slice(),
        [Command::SetQueue { .. }, Command::Play]
    ));
}

#[test]
fn replace_queue_sets_queue_and_plays() {
    let mut state = AppState::new();
    let commands = update(
        &mut state,
        Message::ReplaceQueue {
            items: items(&["a", "b", "c"]),
            start: 1,
        },
    );
    assert_eq!(
        commands,
        vec![
            Command::SetQueue {
                items: ids(&["a", "b", "c"]),
                start: 1
            },
            Command::Play
        ]
    );
    assert_eq!(current_id(&state), "b");
}

#[test]
fn append_while_playing_syncs_upcoming_items() {
    let mut state = state_with(&["a", "b"], 0);
    let _ = event(
        &mut state,
        EngineEvent::State {
            state: PlaybackState::Playing,
        },
    );
    let commands = update(&mut state, Message::Append(items(&["c"])));
    assert_eq!(
        commands,
        vec![Command::SyncUpcoming {
            items: ids(&["a", "b", "c"]),
            current: 0
        }]
    );
}

#[test]
fn append_to_empty_queue_sets_queue_without_play() {
    let mut state = AppState::new();
    let commands = update(&mut state, Message::Append(items(&["a"])));
    assert_eq!(
        commands,
        vec![Command::SetQueue {
            items: ids(&["a"]),
            start: 0
        }]
    );
}

#[test]
fn remove_current_item_while_playing_sets_queue_and_plays() {
    let mut state = state_with(&["a", "b", "c"], 1);
    let _ = event(
        &mut state,
        EngineEvent::State {
            state: PlaybackState::Playing,
        },
    );
    let commands = update(
        &mut state,
        Message::Remove {
            index: 1,
            id: ItemId::new("b"),
        },
    );
    assert_eq!(
        commands,
        vec![
            Command::SetQueue {
                items: ids(&["a", "c"]),
                start: 1
            },
            Command::Play
        ]
    );
}

#[test]
fn jump_to_plays_the_item_and_syncs_upcoming() {
    let mut state = state_with(&["a", "b", "c"], 0);
    let commands = update(
        &mut state,
        Message::JumpTo {
            index: 2,
            id: ItemId::new("c"),
        },
    );
    assert_eq!(
        commands,
        vec![
            Command::PlayItem {
                id: ItemId::new("c")
            },
            Command::SyncUpcoming {
                items: ids(&["a", "b", "c"]),
                current: 2
            },
            Command::Play
        ]
    );
    assert_eq!(current_id(&state), "c");
}

#[test]
fn jump_to_current_item_restarts_it() {
    let mut state = state_with(&["a", "b"], 1);
    assert_eq!(
        update(
            &mut state,
            Message::JumpTo {
                index: 1,
                id: ItemId::new("b")
            }
        ),
        vec![Command::Seek(Duration::ZERO)]
    );
    assert_eq!(
        update(
            &mut state,
            Message::JumpTo {
                index: 9,
                id: ItemId::new("b")
            }
        ),
        Vec::new()
    );
}

#[test]
fn move_upcoming_syncs_upcoming_without_stopping() {
    let mut state = state_with(&["a", "b", "c"], 0);
    let commands = update(
        &mut state,
        Message::MoveUpcoming {
            id: ItemId::new("c"),
            from: 2,
            to: 1,
        },
    );
    assert_eq!(
        commands,
        vec![Command::SyncUpcoming {
            items: ids(&["a", "c", "b"]),
            current: 0
        }]
    );
}

#[test]
fn edits_with_a_stale_item_do_nothing() {
    let mut state = state_with(&["a", "b", "c"], 0);
    let missing_id = ItemId::new("x");
    assert_eq!(
        update(
            &mut state,
            Message::Remove {
                index: 1,
                id: missing_id.clone()
            }
        ),
        Vec::new()
    );
    assert_eq!(
        update(
            &mut state,
            Message::JumpTo {
                index: 1,
                id: missing_id.clone()
            }
        ),
        Vec::new()
    );
    assert_eq!(
        update(
            &mut state,
            Message::MoveUpcoming {
                id: missing_id,
                from: 2,
                to: 1
            }
        ),
        Vec::new()
    );
    assert_eq!(state.queue.items().len(), 3);
    assert_eq!(current_id(&state), "a");
}

#[test]
fn clear_queue_stops_playback() {
    let mut state = state_with(&["a", "b"], 0);
    assert_eq!(update(&mut state, Message::ClearQueue), vec![Command::Stop]);
    assert_eq!(state.queue.items(), Vec::new());
}

#[test]
fn stop_message_stops_the_engine_and_keeps_the_queue() {
    let mut state = state_with(&["a", "b"], 0);
    assert_eq!(update(&mut state, Message::Stop), vec![Command::Stop]);
    assert_eq!(state.queue.items().len(), 2);
}

#[test]
fn remove_last_item_stops_playback() {
    let mut state = state_with(&["a"], 0);
    assert_eq!(
        update(
            &mut state,
            Message::Remove {
                index: 0,
                id: ItemId::new("a")
            }
        ),
        vec![Command::Stop]
    );
}

#[test]
fn next_advances_and_calls_engine() {
    let mut state = state_with(&["a", "b"], 0);
    assert_eq!(update(&mut state, Message::Next), vec![Command::Next]);
    assert_eq!(current_id(&state), "b");
}

#[test]
fn next_at_end_without_repeat_does_nothing() {
    let mut state = state_with(&["a", "b"], 1);
    assert_eq!(update(&mut state, Message::Next), Vec::new());
    assert_eq!(current_id(&state), "b");
}

#[test]
fn next_with_repeat_one_restarts_the_current_item() {
    let mut state = state_with(&["a", "b"], 0);
    let _ = update(&mut state, Message::SetRepeat(RepeatMode::One));
    assert_eq!(
        update(&mut state, Message::Next),
        vec![Command::Seek(Duration::ZERO)]
    );
    assert_eq!(current_id(&state), "a");
}

#[test]
fn previous_at_start_restarts_the_current_item() {
    let mut state = state_with(&["a", "b"], 0);
    assert_eq!(
        update(&mut state, Message::Previous),
        vec![Command::Seek(Duration::ZERO)]
    );
}

#[test]
fn previous_goes_back_and_plays_the_item() {
    let mut state = state_with(&["a", "b"], 1);
    assert_eq!(
        update(&mut state, Message::Previous),
        vec![
            Command::PlayItem {
                id: ItemId::new("a")
            },
            Command::SyncUpcoming {
                items: ids(&["a", "b"]),
                current: 0
            }
        ]
    );
    assert_eq!(current_id(&state), "a");
}

#[test]
fn set_volume_limits_the_value() {
    let mut state = AppState::new();
    assert_eq!(
        update(&mut state, Message::SetVolume(1.7)),
        vec![Command::SetVolume(1.0)]
    );
    assert!((state.playback.volume - 1.0).abs() < f32::EPSILON);
}

#[test]
fn set_shuffle_with_same_seed_gives_same_order() {
    let names = ["a", "b", "c", "d", "e", "f"];
    let shuffled = || {
        let mut state = state_with(&names, 0);
        let _ = update(
            &mut state,
            Message::SetShuffle {
                enabled: true,
                seed: 3,
            },
        );
        state.queue.playback_order()
    };
    assert_eq!(shuffled(), shuffled());
}

#[test]
fn now_playing_event_moves_the_queue_and_resets_position() {
    let mut state = state_with(&["a", "b"], 0);
    let _ = event(
        &mut state,
        EngineEvent::Position {
            position: Duration::from_secs(9),
        },
    );
    let _ = event(
        &mut state,
        EngineEvent::NowPlaying {
            id: ItemId::new("b"),
            title: "title b".to_owned(),
            artist: "artist".to_owned(),
            album: "album".to_owned(),
            duration: Duration::from_secs(180),
            artwork_url: None,
            album_id: None,
            queue_index: 1,
        },
    );
    assert_eq!(current_id(&state), "b");
    assert_eq!(state.playback.position, Duration::ZERO);
    assert_eq!(state.now_playing.map(|now| now.id), Some(ItemId::new("b")));
}

#[test]
fn ready_applied_twice_gives_the_same_status() {
    let mut state = state_with(&["a"], 0);
    let _ = event(&mut state, EngineEvent::Ready);
    assert_eq!(state.engine, EngineStatus::Ready);
    let _ = event(&mut state, EngineEvent::Ready);
    assert_eq!(state.engine, EngineStatus::Ready);
    assert_eq!(current_id(&state), "a");
    assert_eq!(state.resume, None);
}

#[test]
fn ready_after_restart_sets_the_queue_again() {
    let mut state = state_with(&["a", "b"], 1);
    let _ = event(&mut state, EngineEvent::Restarted);
    assert_eq!(state.engine, EngineStatus::Restarting);
    let commands = event(&mut state, EngineEvent::Ready);
    assert_eq!(state.engine, EngineStatus::Ready);
    assert!(commands.contains(&Command::SetQueue {
        items: ids(&["a", "b"]),
        start: 1
    }));
    assert!(!commands.contains(&Command::Play));
}

#[test]
fn ready_after_restart_seeks_and_plays_if_playback_was_running() {
    let mut state = state_with(&["a", "b"], 1);
    let _ = event(
        &mut state,
        EngineEvent::State {
            state: PlaybackState::Playing,
        },
    );
    let _ = event(
        &mut state,
        EngineEvent::Position {
            position: Duration::from_secs(42),
        },
    );
    let _ = event(&mut state, EngineEvent::Restarted);
    // A second restart before the engine is ready must keep the first position.
    let _ = event(&mut state, EngineEvent::Restarted);
    let commands = event(&mut state, EngineEvent::Ready);
    let resume: Vec<_> = commands.into_iter().skip(3).collect();
    assert_eq!(
        resume,
        vec![
            Command::SetQueue {
                items: ids(&["a", "b"]),
                start: 1
            },
            Command::Play,
        ]
    );
    // The seek waits until the engine plays.
    let commands = event(
        &mut state,
        EngineEvent::State {
            state: PlaybackState::Playing,
        },
    );
    assert_eq!(commands, vec![Command::Seek(Duration::from_secs(42))]);
    let commands = event(
        &mut state,
        EngineEvent::State {
            state: PlaybackState::Paused,
        },
    );
    assert_eq!(commands, Vec::new());
}

#[test]
fn new_restart_drops_a_waiting_seek() {
    let mut state = state_with(&["a"], 0);
    state.pending_seek = Some(Duration::from_secs(5));
    let _ = event(&mut state, EngineEvent::Restarted);
    assert_eq!(state.pending_seek, None);
}

#[test]
fn restore_queue_waits_for_the_engine_and_does_not_play() {
    let mut saved = AppState::new();
    let _ = update(
        &mut saved,
        Message::ReplaceQueue {
            items: items(&["a", "b"]),
            start: 1,
        },
    );
    let message = Message::RestoreQueue {
        queue: saved.queue.clone(),
        position: Duration::from_secs(7),
    };
    let mut state = AppState::new();
    assert_eq!(update(&mut state, message), Vec::new());
    assert_eq!(state.queue, saved.queue);
    assert_eq!(state.playback.position, Duration::from_secs(7));

    let commands = event(&mut state, EngineEvent::Ready);

    let resume: Vec<_> = commands.into_iter().skip(3).collect();
    assert_eq!(
        resume,
        vec![
            Command::SetQueue {
                items: ids(&["a", "b"]),
                start: 1
            },
            Command::Seek(Duration::from_secs(7)),
        ]
    );
}

#[test]
fn restore_queue_goes_to_a_ready_engine_at_once() {
    let mut state = AppState::new();
    let _ = event(&mut state, EngineEvent::Ready);
    let mut saved = AppState::new();
    let _ = update(
        &mut saved,
        Message::ReplaceQueue {
            items: items(&["a"]),
            start: 0,
        },
    );
    let commands = update(
        &mut state,
        Message::RestoreQueue {
            queue: saved.queue,
            position: Duration::from_secs(3),
        },
    );
    assert_eq!(
        commands,
        vec![
            Command::SetQueue {
                items: ids(&["a"]),
                start: 0
            },
            Command::Seek(Duration::from_secs(3)),
        ]
    );
}

#[test]
fn first_ready_does_not_set_the_queue() {
    let mut state = state_with(&["a"], 0);
    let commands = event(&mut state, EngineEvent::Ready);
    assert_eq!(
        commands,
        vec![
            Command::RefreshAuth,
            Command::SetVolume(1.0),
            Command::SetRepeat(RepeatMode::None)
        ]
    );
}

#[test]
fn auth_loaded_sets_signed_in_state() {
    let mut state = AppState::new();
    let status = AuthStatus {
        authorized: true,
        storefront: "it".to_owned(),
    };
    let _ = update(&mut state, Message::AuthLoaded(status));
    assert_eq!(
        state.auth,
        AuthState::SignedIn {
            storefront: "it".to_owned()
        }
    );
}

#[test]
fn supervisor_events_set_the_engine_status() {
    let mut state = AppState::new();
    let _ = event(&mut state, EngineEvent::GaveUp);
    assert_eq!(state.engine, EngineStatus::GaveUp);
}

#[test]
fn sandbox_failure_sets_the_engine_status_and_allows_restart() {
    let mut state = AppState::new();
    let _ = event(
        &mut state,
        EngineEvent::Error {
            code: ErrorCode::SandboxFailed,
            message: "sandbox".to_owned(),
            fatal: true,
        },
    );
    assert_eq!(state.engine, EngineStatus::SandboxFailed);

    let commands = update(&mut state, Message::RestartEngine);
    assert_eq!(commands, vec![Command::RestartEngine]);
    assert_eq!(state.engine, EngineStatus::Starting);
}

#[test]
fn protocol_mismatch_sets_the_engine_status_and_allows_restart() {
    let mut state = AppState::new();
    let _ = event(
        &mut state,
        EngineEvent::ProtocolMismatch {
            expected: 1,
            actual: 2,
        },
    );
    assert_eq!(
        state.engine,
        EngineStatus::ProtocolMismatch {
            expected: 1,
            actual: 2
        }
    );

    let commands = update(&mut state, Message::RestartEngine);
    assert_eq!(commands, vec![Command::RestartEngine]);
    assert_eq!(state.engine, EngineStatus::Starting);
}

#[test]
fn sign_in_marks_the_login_as_open() {
    let mut state = AppState::new();
    let commands = update(&mut state, Message::SignIn);
    assert_eq!((commands, state.signing_in), (vec![Command::SignIn], true));
}

#[test]
fn second_sign_in_while_the_login_is_open_does_nothing() {
    let mut state = AppState::new();
    let _ = update(&mut state, Message::SignIn);
    assert_eq!(update(&mut state, Message::SignIn), Vec::new());
}

#[test]
fn sign_in_ended_clears_the_login_flag() {
    let mut state = AppState::new();
    let _ = update(&mut state, Message::SignIn);
    let _ = update(&mut state, Message::SignInEnded { failed: false });
    assert!(!state.signing_in);
}

#[test]
fn no_subscription_message_sets_the_flag() {
    let mut state = AppState::new();
    let _ = update(&mut state, Message::SubscriptionChecked { active: false });
    assert!(state.no_subscription);
}

#[test]
fn sign_out_clears_the_subscription_flag() {
    let mut state = AppState::new();
    let _ = update(&mut state, Message::SubscriptionChecked { active: false });
    let _ = event(&mut state, EngineEvent::AuthChanged { authorized: false });
    assert!(!state.no_subscription);
}

#[test]
fn restart_engine_after_give_up_starts_the_engine_again() {
    let mut state = AppState::new();
    let _ = event(&mut state, EngineEvent::GaveUp);
    let commands = update(&mut state, Message::RestartEngine);
    assert_eq!(
        (commands, state.engine),
        (vec![Command::RestartEngine], EngineStatus::Starting)
    );
}

#[test]
fn restart_engine_does_nothing_while_the_engine_runs() {
    let mut state = AppState::new();
    let _ = event(&mut state, EngineEvent::Ready);
    assert_eq!(update(&mut state, Message::RestartEngine), Vec::new());
}

#[test]
fn active_subscription_check_clears_the_flag() {
    let mut state = AppState::new();
    let _ = update(&mut state, Message::SubscriptionChecked { active: false });
    let _ = update(&mut state, Message::SubscriptionChecked { active: true });
    assert!(!state.no_subscription);
}

#[test]
fn failed_sign_in_sets_the_flag_and_a_new_attempt_clears_it() {
    let mut state = AppState::new();
    let _ = update(&mut state, Message::SignIn);
    let _ = update(&mut state, Message::SignInEnded { failed: true });
    let failed = state.sign_in_failed;
    let _ = update(&mut state, Message::SignIn);
    assert_eq!((failed, state.sign_in_failed), (true, false));
}
