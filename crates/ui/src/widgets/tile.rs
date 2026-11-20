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

/// Size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 140;

/// A titled row of tiles that scrolls sideways. It hides itself when it has
/// no tiles.
pub struct TileRow {
    root: gtk::Box,
    tiles: gtk::Box,
    /// The images must live as long as the tiles: they load in the background.
    images: RefCell<Vec<ArtworkImage>>,
}

impl TileRow {
    /// Makes an empty row with the heading `title`.
    pub fn new(title: &str) -> Self {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .visible(false)
            .build();
        root.append(&heading(title));
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
        while let Some(tile) = self.tiles.first_child() {
            self.tiles.remove(&tile);
        }
        let mut images = self.images.borrow_mut();
        images.clear();
        for item in items {
            let (tile, image) = build_tile(item);
            self.tiles.append(&tile);
            images.push(image);
        }
        self.root.set_visible(!items.is_empty());
    }
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

/// Builds a flat button with the artwork, the title, and the subtitle. A
/// click runs the open action of the kind with the ID of the item.
fn build_tile(item: &TileItem) -> (gtk::Button, ArtworkImage) {
    let image = ArtworkImage::shared();
    image.set_artwork(item.artwork_template.as_deref(), ARTWORK_SIZE);
    if item.kind == TileKind::Artist {
        image.widget().add_css_class("circular");
    }
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .width_request(ARTWORK_SIZE)
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
