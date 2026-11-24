// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Row of the queue view: artwork, title, artist, and duration. A row of the
//! upcoming list also has a drag handle and a remove button.

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::gdk::{ContentProvider, DragAction};
use relm4::gtk::prelude::*;
use relm4::typed_view::list::RelmListItem;
use ringo_core::queue::QueueItem;

use crate::widgets::artwork_image::ArtworkImage;
use crate::widgets::player_bar::format_time;
use crate::widgets::song_row::{row_root, title_artist_texts};

/// Name of the window action that removes an upcoming item. It takes the
/// position in the list.
pub const REMOVE_ACTION: &str = "win.queue-remove";

/// Name of the window action that moves an upcoming item. It takes the old
/// and the new position in the list.
pub const MOVE_ACTION: &str = "win.queue-move";

/// Style classes of the drag and drop. `style.css` defines them.
const DRAGGING_CLASS: &str = "queue-dragging";
const DROP_ABOVE_CLASS: &str = "queue-drop-above";
const DROP_BELOW_CLASS: &str = "queue-drop-below";

/// Where the pointer holds the drag icon, in pixels from the row corner.
const DRAG_HOT_X: i32 = 24;
const DRAG_HOT_Y: i32 = 24;

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
        let root = row_root();
        let artwork = ArtworkImage::shared();
        root.append(artwork.widget());
        let (texts, title, artist) = title_artist_texts();
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
    // The drag icon is the row itself. The row in the list gets dim, so the
    // user sees what moves.
    let row = root.downgrade();
    source.connect_drag_begin(move |source, _| {
        if let Some(row) = row.upgrade() {
            let icon = gtk::WidgetPaintable::new(Some(&row));
            source.set_icon(Some(&icon), DRAG_HOT_X, DRAG_HOT_Y);
            row.add_css_class(DRAGGING_CLASS);
        }
    });
    let row = root.downgrade();
    source.connect_drag_end(move |_, _, _| {
        if let Some(row) = row.upgrade() {
            row.remove_css_class(DRAGGING_CLASS);
        }
    });
    root.add_controller(source);

    let target = gtk::DropTarget::new(u32::static_type(), DragAction::MOVE);
    // A line shows where the dragged item goes: above the row when the item
    // comes from below, and under the row when it comes from above.
    let item = list_item.downgrade();
    let row = root.downgrade();
    target.connect_motion(move |target, _, _| {
        let (Some(item), Some(row)) = (item.upgrade(), row.upgrade()) else {
            return DragAction::empty();
        };
        let from = target.value().and_then(|value| value.get::<u32>().ok());
        show_drop_line(&row, from.map(|from| from < item.position()));
        DragAction::MOVE
    });
    let row = root.downgrade();
    target.connect_leave(move |_| {
        if let Some(row) = row.upgrade() {
            show_drop_line(&row, None);
        }
    });
    let item = list_item.downgrade();
    let row = root.downgrade();
    target.connect_drop(move |_, value, _, _| {
        let (Ok(from), Some(item), Some(row)) = (value.get::<u32>(), item.upgrade(), row.upgrade())
        else {
            return false;
        };
        show_drop_line(&row, None);
        let moved = (from, item.position()).to_variant();
        row.activate_action(MOVE_ACTION, Some(&moved)).is_ok()
    });
    root.add_controller(target);
}

/// Shows the drop line under the row (`Some(true)`), above it
/// (`Some(false)`), or hides it (`None`).
fn show_drop_line(row: &gtk::Box, below: Option<bool>) {
    row.remove_css_class(DROP_ABOVE_CLASS);
    row.remove_css_class(DROP_BELOW_CLASS);
    match below {
        Some(true) => row.add_css_class(DROP_BELOW_CLASS),
        Some(false) => row.add_css_class(DROP_ABOVE_CLASS),
        None => {}
    }
}
