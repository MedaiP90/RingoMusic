// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Songs in a horizontal carousel, like Apple Music. Each column holds a few
//! songs. The carousel scrolls sideways and has the height of one column.

use std::cell::RefCell;
use std::rc::Rc;

use relm4::gtk;
use relm4::gtk::prelude::*;

use crate::item::SongItem;
use crate::widgets::song_list::SongList;
use crate::widgets::song_row::Lead;

/// Number of songs in a full column.
const ROWS_PER_COLUMN: usize = 4;

/// Width of a column in pixels. The narrow layout shows one column.
const COLUMN_WIDTH: i32 = 340;

/// A row of song columns that scrolls sideways.
pub struct SongCarousel {
    scroller: gtk::ScrolledWindow,
    columns: gtk::Box,
    /// The lists own the rows and their images, so they live as long as the columns.
    lists: RefCell<Vec<SongList>>,
    on_activated: Rc<dyn Fn(usize)>,
}

impl SongCarousel {
    /// Makes an empty carousel. `on_activated` gets the position of the song
    /// in the whole carousel, not in its column.
    pub fn new(on_activated: impl Fn(usize) + 'static) -> Self {
        let columns = gtk::Box::builder()
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .child(&columns)
            .build();
        Self {
            scroller,
            columns,
            lists: RefCell::new(Vec::new()),
            on_activated: Rc::new(on_activated),
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scroller
    }

    /// Replaces the columns with `songs`, in order, `ROWS_PER_COLUMN` songs
    /// for each column.
    pub fn set_songs(&self, songs: &[SongItem]) {
        while let Some(column) = self.columns.first_child() {
            self.columns.remove(&column);
        }
        let mut lists = self.lists.borrow_mut();
        lists.clear();
        for (index, chunk) in songs.chunks(ROWS_PER_COLUMN).enumerate() {
            let first = index * ROWS_PER_COLUMN;
            let on_activated = Rc::clone(&self.on_activated);
            let list = SongList::new(move |position| on_activated(first + position));
            list.set_songs(chunk.iter().map(|song| (song, Lead::Artwork)));
            list.widget().set_width_request(COLUMN_WIDTH);
            list.widget().set_valign(gtk::Align::Start);
            self.columns.append(list.widget());
            lists.push(list);
        }
    }
}
