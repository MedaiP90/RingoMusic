// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Window actions for playback and their keyboard shortcuts. Buttons, menus,
//! and keys all call these actions.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::gio::SimpleAction;
use relm4::gtk::glib::{self, Variant};
use relm4::gtk::prelude::*;
use ringo_core::runner::CoreHandle;
use ringo_core::state::Message;
use ringo_engine::protocol::{PlaybackState, RepeatMode};

/// An action with its keyboard shortcut.
#[derive(Copy, Clone, Debug)]
pub struct Shortcut {
    /// Full action name, with the `win.` prefix.
    pub action: &'static str,
    /// Accelerator in the GTK format.
    pub accelerator: &'static str,
}

pub const PLAY_PAUSE: Shortcut = Shortcut {
    action: "win.play-pause",
    accelerator: "<Ctrl>space",
};
pub const NEXT: Shortcut = Shortcut {
    action: "win.next",
    accelerator: "<Ctrl>Right",
};
pub const PREVIOUS: Shortcut = Shortcut {
    action: "win.previous",
    accelerator: "<Ctrl>Left",
};
pub const TOGGLE_SHUFFLE: Shortcut = Shortcut {
    action: "win.toggle-shuffle",
    accelerator: "<Ctrl>s",
};
pub const CYCLE_REPEAT: Shortcut = Shortcut {
    action: "win.cycle-repeat",
    accelerator: "<Ctrl>r",
};

/// Action that opens the album of the current song.
pub const OPEN_ALBUM: &str = "win.open-album";

impl Shortcut {
    /// Returns `label` with the shortcut, for a tooltip. The shortcut is
    /// written for the user, for example "Ctrl+Space".
    pub fn tooltip(self, label: &str) -> String {
        gettext("{action} ({shortcut})")
            .replace("{action}", label)
            .replace("{shortcut}", &self.label())
    }

    fn label(self) -> String {
        gtk::accelerator_parse(self.accelerator).map_or_else(
            || self.accelerator.to_owned(),
            |(key, modifiers)| gtk::accelerator_get_label(key, modifiers).to_string(),
        )
    }
}

/// The part of the state that the actions follow.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ActionState {
    /// The queue has items, so the transport actions are possible.
    pub has_queue: bool,
    /// Shuffle is on.
    pub shuffle: bool,
}

/// The playback actions of one window.
#[derive(Clone, Debug)]
pub struct PlaybackActions {
    transport: [SimpleAction; 3],
    shuffle: SimpleAction,
    repeat: SimpleAction,
}

impl PlaybackActions {
    /// Adds the actions to `window` and sets their shortcuts. The actions send
    /// messages to `core`. They are off until [`sync`](Self::sync) turns them on.
    pub fn register(window: &adw::ApplicationWindow, core: &CoreHandle) -> Self {
        let actions = Self {
            transport: [
                play_pause_action(core),
                message_action("next", core, Message::Next),
                message_action("previous", core, Message::Previous),
            ],
            shuffle: shuffle_action(core),
            repeat: repeat_action(core),
        };
        for action in actions.all() {
            window.add_action(action);
        }
        window.add_action(&disabled_open_album_action());
        let application = relm4::main_application();
        for shortcut in [PLAY_PAUSE, NEXT, PREVIOUS, TOGGLE_SHUFFLE, CYCLE_REPEAT] {
            application.set_accels_for_action(shortcut.action, &[shortcut.accelerator]);
        }
        actions
    }

    /// Enables or disables the actions and shows the shuffle setting.
    pub fn sync(&self, state: ActionState) {
        for action in self.all() {
            action.set_enabled(state.has_queue);
        }
        self.shuffle.set_state(&state.shuffle.to_variant());
    }

    fn all(&self) -> impl Iterator<Item = &SimpleAction> {
        self.transport.iter().chain([&self.shuffle, &self.repeat])
    }
}

/// Returns the repeat mode after `mode`: none, all, one, and none again.
pub fn next_repeat(mode: RepeatMode) -> RepeatMode {
    match mode {
        RepeatMode::None => RepeatMode::All,
        RepeatMode::All => RepeatMode::One,
        RepeatMode::One => RepeatMode::None,
    }
}

/// Sends `message` to the core without blocking the main thread.
fn send(core: &CoreHandle, message: Message) {
    let core = core.clone();
    relm4::spawn(async move { core.send(message).await });
}

/// Creates an action that sends `message`.
fn message_action(name: &str, core: &CoreHandle, message: Message) -> SimpleAction {
    let action = SimpleAction::new(name, None);
    let core = core.clone();
    action.connect_activate(move |_, _| send(&core, message.clone()));
    action
}

/// Creates the action that pauses a playing song and plays a paused one.
fn play_pause_action(core: &CoreHandle) -> SimpleAction {
    let action = SimpleAction::new("play-pause", None);
    let core = core.clone();
    let state = core.subscribe();
    action.connect_activate(move |_, _| {
        let playing = state.borrow().playback.state == PlaybackState::Playing;
        send(
            &core,
            if playing {
                Message::Pause
            } else {
                Message::Play
            },
        );
    });
    action
}

/// Creates the shuffle action. The core owns the shuffle setting, so the
/// state of the action only changes with [`PlaybackActions::sync`].
fn shuffle_action(core: &CoreHandle) -> SimpleAction {
    let action = SimpleAction::new_stateful("toggle-shuffle", None, &false.to_variant());
    let core = core.clone();
    action.connect_change_state(move |_, requested| {
        if let Some(enabled) = requested.and_then(Variant::get::<bool>) {
            let seed = random_seed();
            send(&core, Message::SetShuffle { enabled, seed });
        }
    });
    action
}

/// Creates the action that goes to the next repeat mode.
fn repeat_action(core: &CoreHandle) -> SimpleAction {
    let action = SimpleAction::new("cycle-repeat", None);
    let core = core.clone();
    let state = core.subscribe();
    action.connect_activate(move |_, _| {
        let mode = next_repeat(state.borrow().queue.repeat());
        send(&core, Message::SetRepeat(mode));
    });
    action
}

/// Creates the action for the title of the song. It stays off until the album
/// view exists.
// ponytail: prompt 32 gives this action a handler and turns it on.
fn disabled_open_album_action() -> SimpleAction {
    let action = SimpleAction::new("open-album", None);
    action.set_enabled(false);
    action
}

/// Returns a random seed for the shuffle order.
fn random_seed() -> u64 {
    (u64::from(glib::random_int()) << 32) | u64::from(glib::random_int())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_cycles_through_all_modes() {
        let first = next_repeat(RepeatMode::None);
        let second = next_repeat(first);
        let third = next_repeat(second);
        assert_eq!(
            (first, second, third),
            (RepeatMode::All, RepeatMode::One, RepeatMode::None)
        );
    }
}
