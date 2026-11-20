// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! A short list of songs in a list box. It does not scroll by itself. Use it
//! inside a page that scrolls, for lists of a few hundred songs at most.

use std::cell::RefCell;

use relm4::gtk;
use relm4::gtk::prelude::*;

use crate::item::SongItem;
use crate::widgets::song_row::{self, Lead, SongRowWidgets};

/// A list of song rows. A press on a row reports its position.
pub struct SongList {
    list: gtk::ListBox,
    /// The rows load artwork in the background, so they live as long as the list.
    rows: RefCell<Vec<SongRowWidgets>>,
}

impl SongList {
    /// Makes an empty list. `on_activated` gets the position of the row.
    pub fn new(on_activated: impl Fn(usize) + 'static) -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .show_separators(true)
            .css_classes(["song-list"])
            .build();
        list.connect_row_activated(move |_, row| {
            if let Ok(position) = usize::try_from(row.index()) {
                on_activated(position);
            }
        });
        song_row::connect_menu_key(&list);
        Self {
            list,
            rows: RefCell::new(Vec::new()),
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::ListBox {
        &self.list
    }

    /// Replaces the rows with `songs`. Each song has its own lead.
    pub fn set_songs<'a>(&self, songs: impl IntoIterator<Item = (&'a SongItem, Lead)>) {
        self.list.remove_all();
        self.rows.borrow_mut().clear();
        self.append_songs(songs);
    }

    /// Adds `songs` after the rows that the list has.
    pub fn append_songs<'a>(&self, songs: impl IntoIterator<Item = (&'a SongItem, Lead)>) {
        let mut rows = self.rows.borrow_mut();
        for (song, lead) in songs {
            let (root, widgets) = SongRowWidgets::build(None);
            widgets.show(song, lead);
            self.list.append(&root);
            rows.push(widgets);
        }
    }
}
