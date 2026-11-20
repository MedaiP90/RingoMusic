// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Window actions that every item widget uses. Each action takes the ID of
//! the item as a string parameter.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk::gio::SimpleAction;
use relm4::gtk::glib::{Variant, VariantTy};
use relm4::gtk::prelude::*;
use ringo_core::queue::QueueItem;
use ringo_core::runner::CoreHandle;
use ringo_core::state::Message;

use crate::item::SongItem;

pub const PLAY_ITEM: &str = "win.play-item";
pub const PLAY_NEXT: &str = "win.play-next";
pub const ADD_TO_QUEUE: &str = "win.add-to-queue";
pub const OPEN_ALBUM: &str = "win.open-album";
pub const OPEN_ARTIST: &str = "win.open-artist";

/// The songs that the views showed, by ID. An action gets only an ID, so it
/// finds the data of the song here. Clones share the same map.
// ponytail: the map only grows. Add a limit if the library views make it big.
#[derive(Clone, Debug, Default)]
pub struct SongCatalog {
    songs: Rc<RefCell<HashMap<String, QueueItem>>>,
}

impl SongCatalog {
    /// Remembers `songs`. A known ID gets the new data.
    pub fn remember(&self, songs: &[SongItem]) {
        let mut known = self.songs.borrow_mut();
        for song in songs {
            known.insert(song.id().to_owned(), song.queue.clone());
        }
    }

    fn find(&self, id: &str) -> Option<QueueItem> {
        self.songs.borrow().get(id).cloned()
    }
}

/// What a song action sends to the core.
#[derive(Copy, Clone, Debug)]
enum SongCommand {
    Play,
    PlayNext,
    AddToQueue,
}

impl SongCommand {
    fn message(self, item: QueueItem) -> Message {
        match self {
            Self::Play => Message::ReplaceQueue {
                items: vec![item],
                start: 0,
            },
            Self::PlayNext => Message::PlayNext(vec![item]),
            Self::AddToQueue => Message::Append(vec![item]),
        }
    }
}

/// Adds the item actions to `window`. `show_toast` shows a short message.
pub fn register(
    window: &adw::ApplicationWindow,
    core: &CoreHandle,
    catalog: &SongCatalog,
    show_toast: impl Fn(String) + Clone + 'static,
) {
    let song_actions = [
        ("play-item", SongCommand::Play),
        ("play-next", SongCommand::PlayNext),
        ("add-to-queue", SongCommand::AddToQueue),
    ];
    for (name, command) in song_actions {
        window.add_action(&song_action(name, command, core, catalog));
    }
    // ponytail: prompt 32 gives the detail pages and replaces this toast.
    for name in ["open-album", "open-artist", "open-playlist"] {
        let show_toast = show_toast.clone();
        let action = item_action(name);
        action.connect_activate(move |_, _| show_toast(gettext("Not available yet")));
        window.add_action(&action);
    }
}

/// Creates an action with the item ID as parameter.
fn item_action(name: &str) -> SimpleAction {
    SimpleAction::new(name, Some(VariantTy::STRING))
}

/// Creates an action that sends `command` for the song of the ID.
fn song_action(
    name: &str,
    command: SongCommand,
    core: &CoreHandle,
    catalog: &SongCatalog,
) -> SimpleAction {
    let action = item_action(name);
    let (core, catalog) = (core.clone(), catalog.clone());
    action.connect_activate(move |_, parameter| {
        let Some(item) = parameter
            .and_then(Variant::str)
            .and_then(|id| catalog.find(id))
        else {
            return;
        };
        let (core, message) = (core.clone(), command.message(item));
        relm4::spawn(async move { core.send(message).await });
    });
    action
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ringo_engine::ItemId;

    use super::*;

    fn song(id: &str, title: &str) -> SongItem {
        SongItem {
            queue: QueueItem {
                id: ItemId::new(id),
                title: title.to_owned(),
                artist: String::new(),
                album: String::new(),
                duration: Duration::ZERO,
                artwork_template: None,
            },
            explicit: false,
            album_id: None,
            artist_id: None,
        }
    }

    #[test]
    fn find_returns_a_remembered_song() {
        let catalog = SongCatalog::default();
        catalog.remember(&[song("1", "First")]);
        assert_eq!(
            catalog.find("1").map(|item| item.title).as_deref(),
            Some("First")
        );
    }

    #[test]
    fn find_returns_none_for_an_unknown_id() {
        assert_eq!(SongCatalog::default().find("1"), None);
    }

    #[test]
    fn remember_replaces_the_data_of_a_known_id() {
        let catalog = SongCatalog::default();
        catalog.remember(&[song("1", "Old")]);
        catalog.remember(&[song("1", "New")]);
        assert_eq!(
            catalog.find("1").map(|item| item.title).as_deref(),
            Some("New")
        );
    }

    #[test]
    fn play_replaces_the_queue_with_the_song() {
        let item = song("1", "First").queue;
        let message = SongCommand::Play.message(item.clone());
        assert_eq!(
            message,
            Message::ReplaceQueue {
                items: vec![item],
                start: 0
            }
        );
    }
}
