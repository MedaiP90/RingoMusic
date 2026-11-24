// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tiles for albums, artists, and playlists, and the horizontal row that
//! holds them. One tile type serves the three kinds: only the open action and
//! the artwork shape differ.

use std::cell::RefCell;

use relm4::gtk;
use relm4::gtk::pango::EllipsizeMode;
use relm4::gtk::prelude::*;

use crate::item::{TileItem, TileKind};
use crate::widgets::artwork_image::ArtworkImage;

/// Default size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 140;

/// A titled row of tiles that scrolls sideways. It hides itself when it has
/// no tiles.
pub struct TileRow {
    root: gtk::Box,
    /// The heading and the buttons at its right.
    title_row: gtk::Box,
    tiles: gtk::Box,
    /// Size of the artwork of each tile, in pixels.
    artwork_size: i32,
    /// The images must live as long as the tiles: they load in the background.
    images: RefCell<Vec<ArtworkImage>>,
}

impl TileRow {
    /// Makes an empty row with the heading `title` and tiles of the default
    /// size.
    pub fn new(title: &str) -> Self {
        Self::new_with_artwork_size(title, ARTWORK_SIZE)
    }

    /// Makes an empty row with the heading `title`. The artwork of each tile
    /// is `artwork_size` pixels.
    pub fn new_with_artwork_size(title: &str, artwork_size: i32) -> Self {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .visible(false)
            .build();
        let title_row = heading_row(title);
        root.append(&title_row);
        let tiles = gtk::Box::builder()
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .child(&tiles)
            .build();
        root.append(&scroller);
        Self {
            root,
            title_row,
            tiles,
            artwork_size,
            images: RefCell::new(Vec::new()),
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Replaces the tiles with `items`.
    pub fn set_items(&self, items: &[TileItem]) {
        while let Some(tile) = self.tiles.first_child() {
            self.tiles.remove(&tile);
        }
        let mut images = self.images.borrow_mut();
        images.clear();
        for item in items {
            let (tile, image) = build_tile(item, self.artwork_size);
            self.tiles.append(&tile);
            images.push(image);
        }
        self.root.set_visible(!items.is_empty());
    }

    /// Adds `button` at the right end of the heading.
    pub fn append_header_button(&self, button: &impl IsA<gtk::Widget>) {
        self.title_row.append(button);
    }
}

/// A titled grid of tiles. It wraps to the width of the page and does not
/// scroll by itself. It hides itself when it has no tiles.
pub struct TileGrid {
    root: gtk::Box,
    tiles: gtk::FlowBox,
    /// The images must live as long as the tiles: they load in the background.
    images: RefCell<Vec<ArtworkImage>>,
}

impl TileGrid {
    /// Makes an empty grid with the heading `title`.
    pub fn new(title: &str) -> Self {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .visible(false)
            .build();
        root.append(&heading(title));
        let tiles = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .valign(gtk::Align::Start)
            .row_spacing(6)
            .column_spacing(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        root.append(&tiles);
        Self {
            root,
            tiles,
            images: RefCell::new(Vec::new()),
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Replaces the tiles with `items`.
    pub fn set_items(&self, items: &[TileItem]) {
        self.tiles.remove_all();
        self.images.borrow_mut().clear();
        self.append_items(items);
    }

    /// Adds `items` after the tiles that the grid has.
    pub fn append_items(&self, items: &[TileItem]) {
        let mut images = self.images.borrow_mut();
        for item in items {
            let (tile, image) = build_tile(item, ARTWORK_SIZE);
            self.tiles.append(&tile);
            images.push(image);
        }
        self.root.set_visible(self.tiles.first_child().is_some());
    }
}

/// Builds a row with the heading `title`. The heading fills the row, so a
/// button that the caller appends goes to the right end.
pub fn heading_row(title: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let label = heading(title);
    label.set_hexpand(true);
    row.append(&label);
    row
}

/// Builds the heading of a section.
pub fn heading(title: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .css_classes(["title-4"])
        .build()
}

/// Builds a flat button with the artwork of `artwork_size` pixels, the title,
/// and the subtitle. A click runs the open action of the kind with the ID of
/// the item.
fn build_tile(item: &TileItem, artwork_size: i32) -> (gtk::Button, ArtworkImage) {
    let image = ArtworkImage::shared();
    image.set_artwork(item.artwork_template.as_deref(), artwork_size);
    if item.kind == TileKind::Artist {
        image.widget().add_css_class("circular");
    }
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .width_request(artwork_size)
        .build();
    content.append(image.widget());
    content.append(&text_label(&item.title, &["heading"]));
    if !item.subtitle.is_empty() {
        content.append(&text_label(&item.subtitle, &["dim-label", "caption"]));
    }
    let button = gtk::Button::builder()
        .child(&content)
        .css_classes(["flat"])
        .tooltip_text(&item.title)
        .action_name(item.kind.open_action())
        .action_target(&item.id.to_variant())
        .build();
    (button, image)
}

pub fn text_label(text: &str, css_classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .max_width_chars(14)
        .ellipsize(EllipsizeMode::End)
        .css_classes(css_classes)
        .build()
}
