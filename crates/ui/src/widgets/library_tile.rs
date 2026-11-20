// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Items of the library lists: a tile for a grid, and an artist row for a
//! list. The list view handles the activation, so the items have no button.

use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::typed_view::grid::RelmGridItem;
use relm4::typed_view::list::RelmListItem;

use crate::item::{TileItem, TileKind};
use crate::widgets::artwork_image::ArtworkImage;
use crate::widgets::tile::text_label;

/// Size of the artwork of a tile in pixels.
const TILE_ARTWORK_SIZE: i32 = 160;

/// Size of the artwork of an artist row in pixels.
const ROW_ARTWORK_SIZE: i32 = 40;

/// An album or a playlist in a grid.
#[derive(Debug)]
pub struct GridTile {
    item: TileItem,
}

impl GridTile {
    pub fn new(item: TileItem) -> Self {
        Self { item }
    }
}

/// The widgets of one tile. A grid reuses them for other items.
pub struct GridTileWidgets {
    artwork: ArtworkImage,
    title: gtk::Label,
    subtitle: gtk::Label,
}

impl RelmGridItem for GridTile {
    type Root = gtk::Box;
    type Widgets = GridTileWidgets;

    fn setup(_list_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .width_request(TILE_ARTWORK_SIZE)
            .build();
        let artwork = ArtworkImage::shared();
        let title = text_label("", &["heading"]);
        let subtitle = text_label("", &["dim-label", "caption"]);
        root.append(artwork.widget());
        root.append(&title);
        root.append(&subtitle);
        (
            root,
            GridTileWidgets {
                artwork,
                title,
                subtitle,
            },
        )
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        let item = &self.item;
        widgets.title.set_label(&item.title);
        widgets.title.set_tooltip_text(Some(&item.title));
        widgets.subtitle.set_label(&item.subtitle);
        widgets.subtitle.set_visible(!item.subtitle.is_empty());
        widgets
            .artwork
            .set_artwork(item.artwork_template.as_deref(), TILE_ARTWORK_SIZE);
    }
}

/// An artist in a list: a round picture and the name.
#[derive(Debug)]
pub struct ArtistRow {
    item: TileItem,
}

impl ArtistRow {
    pub fn new(item: TileItem) -> Self {
        debug_assert_eq!(item.kind, TileKind::Artist, "the row shows artists");
        Self { item }
    }
}

/// The widgets of one artist row. A list reuses them for other items.
pub struct ArtistRowWidgets {
    artwork: ArtworkImage,
    name: gtk::Label,
}

impl RelmListItem for ArtistRow {
    type Root = gtk::Box;
    type Widgets = ArtistRowWidgets;

    fn setup(_list_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        let root = gtk::Box::builder()
            .spacing(12)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        let artwork = ArtworkImage::shared();
        artwork.widget().add_css_class("circular");
        let name = text_label("", &[]);
        name.set_valign(gtk::Align::Center);
        name.set_hexpand(true);
        name.set_max_width_chars(-1);
        root.append(artwork.widget());
        root.append(&name);
        (root, ArtistRowWidgets { artwork, name })
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        widgets.name.set_label(&self.item.title);
        widgets.name.set_tooltip_text(Some(&self.item.title));
        widgets
            .artwork
            .set_artwork(self.item.artwork_template.as_deref(), ROW_ARTWORK_SIZE);
    }
}
