// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Row of the queue view: artwork, title, artist, and duration. A row of the
//! upcoming list also has a drag handle and a remove button.

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::gdk::{ContentProvider, DragAction};
use relm4::gtk::pango::EllipsizeMode;
use relm4::gtk::prelude::*;
use relm4::typed_view::list::RelmListItem;
use ringo_core::queue::QueueItem;

use crate::widgets::artwork_image::ArtworkImage;
use crate::widgets::player_bar::format_time;

/// Name of the window action that removes an upcoming item. It takes the
/// position in the list.
pub const REMOVE_ACTION: &str = "win.queue-remove";

/// Name of the window action that moves an upcoming item. It takes the old
/// and the new position in the list.
pub const MOVE_ACTION: &str = "win.queue-move";

/// Size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 40;

/// An item of the upcoming list.
#[derive(Debug)]
pub struct QueueRow {
    item: QueueItem,
}

impl QueueRow {
    pub fn new(item: QueueItem) -> Self {
        Self { item }
    }
}

/// The widgets of one row. A list reuses them for other items.
pub struct QueueRowWidgets {
    artwork: ArtworkImage,
    title: gtk::Label,
    artist: gtk::Label,
    duration: gtk::Label,
}

impl RelmListItem for QueueRow {
    type Root = gtk::Box;
    type Widgets = QueueRowWidgets;

    fn setup(list_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        let (root, widgets) = QueueRowWidgets::build();
        add_edit_controls(&root, list_item);
        (root, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        widgets.show(&self.item);
    }
}

impl QueueRowWidgets {
    /// Builds an empty row. Call [`Self::show`] to fill it.
    pub fn build() -> (gtk::Box, Self) {
        let root = gtk::Box::builder()
            .spacing(12)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        let artwork = ArtworkImage::shared();
        root.append(artwork.widget());
        let texts = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();
        let title = text_label(&[]);
        let artist = text_label(&["dim-label", "caption"]);
        texts.append(&title);
        texts.append(&artist);
        root.append(&texts);
        let duration = gtk::Label::builder()
            .css_classes(["dim-label", "numeric"])
            .build();
        root.append(&duration);
        let widgets = Self {
            artwork,
            title,
            artist,
            duration,
        };
        (root, widgets)
    }

    /// Shows `item` in the row.
    pub fn show(&self, item: &QueueItem) {
        self.title.set_label(&item.title);
        self.title.set_tooltip_text(Some(&item.title));
        self.artist.set_label(&item.artist);
        self.duration.set_label(&format_time(item.duration));
        self.artwork
            .set_artwork(item.artwork_template.as_deref(), ARTWORK_SIZE);
    }
}

fn text_label(css_classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(EllipsizeMode::End)
        .css_classes(css_classes)
        .build()
}

/// Adds the drag handle, the remove button, and the drag and drop of the
/// row. The row of a list view does not know its position: it asks
/// `list_item` when the user acts.
fn add_edit_controls(root: &gtk::Box, list_item: &gtk::ListItem) {
    let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
    handle.add_css_class("dim-label");
    root.prepend(&handle);

    let remove = gtk::Button::builder()
        .icon_name("list-remove-symbolic")
        .tooltip_text(gettext("Remove from Queue"))
        .valign(gtk::Align::Center)
        .css_classes(["flat", "circular"])
        .build();
    let item = list_item.downgrade();
    remove.connect_clicked(move |button| {
        if let Some(item) = item.upgrade() {
            let _ = button.activate_action(REMOVE_ACTION, Some(&item.position().to_variant()));
        }
    });
    root.append(&remove);

    let source = gtk::DragSource::builder().actions(DragAction::MOVE).build();
    let item = list_item.downgrade();
    source.connect_prepare(move |_, _, _| {
        let position = item.upgrade()?.position();
        Some(ContentProvider::for_value(&position.to_value()))
    });
    root.add_controller(source);

    let target = gtk::DropTarget::new(u32::static_type(), DragAction::MOVE);
    let item = list_item.downgrade();
    let row = root.downgrade();
    target.connect_drop(move |_, value, _, _| {
        let (Ok(from), Some(item), Some(row)) = (value.get::<u32>(), item.upgrade(), row.upgrade())
        else {
            return false;
        };
        let moved = (from, item.position()).to_variant();
        row.activate_action(MOVE_ACTION, Some(&moved)).is_ok()
    });
    root.add_controller(target);
}
