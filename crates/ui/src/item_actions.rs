// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Window actions that every item widget uses. Each action takes the ID of
//! the item as a string parameter.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use relm4::adw;
use relm4::gtk::gio::SimpleAction;
use relm4::gtk::glib::{Variant, VariantTy};
use relm4::gtk::prelude::*;
use ringo_core::queue::QueueItem;
use ringo_core::runner::CoreHandle;
use ringo_core::state::Message;

use crate::item::SongItem;
use crate::playback_actions;

pub const PLAY_ITEM: &str = "win.play-item";
pub const PLAY_NEXT: &str = "win.play-next";
pub const ADD_TO_QUEUE: &str = "win.add-to-queue";
pub const OPEN_ALBUM: &str = "win.open-album";
pub const OPEN_ARTIST: &str = "win.open-artist";
/// Opens the page with all top songs of an artist.
pub const OPEN_ARTIST_SONGS: &str = "win.open-artist-songs";
/// Opens the page with all albums of an artist.
pub const OPEN_ARTIST_ALBUMS: &str = "win.open-artist-albums";

/// The songs that the views showed, by ID. An action gets only an ID, so it
/// finds the data of the song here. Clones share the same map.
// ponytail: the map only grows. Add a limit if the library views make it big.
#[derive(Clone, Debug, Default)]
pub struct SongCatalog {
    songs: Rc<RefCell<HashMap<String, KnownSong>>>,
    /// Links that the API gave for a song that no view showed.
    links: Rc<RefCell<HashMap<String, SongLinks>>>,
}

/// The album and the artist of a song.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SongLinks {
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
}

/// What the catalog keeps of a song.
#[derive(Clone, Debug)]
struct KnownSong {
    queue: QueueItem,
    album_id: Option<String>,
    artist_id: Option<String>,
}

impl SongCatalog {
    /// Remembers `songs`. A known ID gets the new data.
    pub fn remember(&self, songs: &[SongItem]) {
        let mut known = self.songs.borrow_mut();
        for song in songs {
            let entry = KnownSong {
                queue: song.queue.clone(),
                album_id: song.album_id.clone(),
                artist_id: song.artist_id.clone(),
            };
            known.insert(song.id().to_owned(), entry);
        }
    }

    fn find(&self, id: &str) -> Option<QueueItem> {
        self.songs.borrow().get(id).map(|known| known.queue.clone())
    }

    /// Remembers the links that the API gave for the song of `id`.
    pub fn remember_links(&self, id: &str, links: SongLinks) {
        self.links.borrow_mut().insert(id.to_owned(), links);
    }

    /// The album of a song, if a view or the API gave it.
    pub fn album_id(&self, id: &str) -> Option<String> {
        let from_view = self
            .songs
            .borrow()
            .get(id)
            .and_then(|known| known.album_id.clone());
        from_view.or_else(|| self.links.borrow().get(id)?.album_id.clone())
    }

    /// The artist of a song, if a view or the API gave it.
    pub fn artist_id(&self, id: &str) -> Option<String> {
        let from_view = self
            .songs
            .borrow()
            .get(id)
            .and_then(|known| known.artist_id.clone());
        from_view.or_else(|| self.links.borrow().get(id)?.artist_id.clone())
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

/// Plays `songs` as the new queue, from the song at `start`. It does nothing
/// if `start` is not a position in `songs`.
pub fn play_songs(core: &CoreHandle, songs: &[SongItem], start: usize) {
    if start >= songs.len() {
        return;
    }
    send(
        core,
        Message::ReplaceQueue {
            items: queue_items(songs),
            start,
        },
    );
}

/// Plays `songs` in random order as the new queue. The core gets one message,
/// so the engine gets one new queue.
pub fn shuffle_songs(core: &CoreHandle, songs: &[SongItem]) {
    if songs.is_empty() {
        return;
    }
    send(
        core,
        Message::ReplaceQueueShuffled {
            items: queue_items(songs),
            seed: playback_actions::random_seed(),
        },
    );
}

fn queue_items(songs: &[SongItem]) -> Vec<QueueItem> {
    songs.iter().map(|song| song.queue.clone()).collect()
}

fn send(core: &CoreHandle, message: Message) {
    let core = core.clone();
    relm4::spawn(async move { core.send(message).await });
}

/// Adds the song actions to `window`. The detail pages add the open actions.
pub fn register(window: &adw::ApplicationWindow, core: &CoreHandle, catalog: &SongCatalog) {
    let song_actions = [
        ("play-item", SongCommand::Play),
        ("play-next", SongCommand::PlayNext),
        ("add-to-queue", SongCommand::AddToQueue),
    ];
    for (name, command) in song_actions {
        window.add_action(&song_action(name, command, core, catalog));
    }
}

/// Creates an action with the item ID as parameter.
pub fn item_action(name: &str) -> SimpleAction {
    SimpleAction::new(name, Some(VariantTy::STRING))
}

/// Creates an action that opens a list of an artist. Its parameter is the
/// artist ID and the artist name. Build it with [`artist_list_target`].
pub fn artist_list_action(name: &str) -> SimpleAction {
    SimpleAction::new(name, Some(&<(String, String)>::static_variant_type()))
}

/// The parameter of an artist list action.
pub fn artist_list_target(artist_id: &str, artist_name: &str) -> Variant {
    (artist_id, artist_name).to_variant()
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
