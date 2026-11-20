// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Row of a song list: artwork, title, artist, duration, and a context menu.

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::gdk::{Key, ModifierType, Rectangle};
use relm4::gtk::gio::{Menu, MenuItem, SimpleAction, SimpleActionGroup};
use relm4::gtk::glib::{Propagation, Variant};
use relm4::gtk::pango::EllipsizeMode;
use relm4::gtk::prelude::*;
use relm4::typed_view::list::RelmListItem;

use crate::item::SongItem;
use crate::item_actions::{ADD_TO_QUEUE, OPEN_ALBUM, OPEN_ARTIST, PLAY_ITEM, PLAY_NEXT};
use crate::name_filter::NameFilter;
use crate::widgets::artwork_image::ArtworkImage;
use crate::widgets::player_bar::format_time;

/// Size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 40;

/// Name of the action group of a row, and of its menu action.
const ROW_GROUP: &str = "row";
const POPUP_ACTION: &str = "popup";

/// An item of a song list.
#[derive(Debug)]
pub struct SongRow {
    item: SongItem,
}

impl SongRow {
    pub fn new(item: SongItem) -> Self {
        Self { item }
    }

    /// Tells if the song matches `filter`.
    pub fn matches(&self, filter: &NameFilter) -> bool {
        self.item.matches(filter)
    }
}

/// The widgets of one row. A list reuses them for other songs.
pub struct SongRowWidgets {
    artwork: ArtworkImage,
    title: gtk::Label,
    artist: gtk::Label,
    duration: gtk::Label,
    explicit: gtk::Label,
    menu: gtk::PopoverMenu,
}

impl RelmListItem for SongRow {
    type Root = gtk::Box;
    type Widgets = SongRowWidgets;

    fn setup(_list_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
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

        let explicit = gtk::Label::builder()
            .label("E")
            .tooltip_text(gettext("Explicit"))
            .css_classes(["dim-label", "caption-heading"])
            .visible(false)
            .build();
        let duration = gtk::Label::builder()
            .css_classes(["dim-label", "numeric"])
            .build();
        root.append(&explicit);
        root.append(&duration);

        let menu = gtk::PopoverMenu::from_model(None::<&Menu>);
        menu.set_parent(&root);
        menu.set_has_arrow(false);
        menu.set_halign(gtk::Align::Start);
        connect_menu_triggers(&root, &menu);
        let widgets = SongRowWidgets {
            artwork,
            title,
            artist,
            duration,
            explicit,
            menu,
        };
        (root, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        let queue = &self.item.queue;
        widgets.title.set_label(&queue.title);
        widgets.title.set_tooltip_text(Some(&queue.title));
        widgets.artist.set_label(&queue.artist);
        widgets.duration.set_label(&format_time(queue.duration));
        widgets.explicit.set_visible(self.item.explicit);
        widgets
            .artwork
            .set_artwork(queue.artwork_template.as_deref(), ARTWORK_SIZE);
        widgets.menu.set_menu_model(Some(&build_menu(&self.item)));
    }
}

/// Lets the Menu key and Shift+F10 open the menu of the focused row of
/// `view`. The row widgets cannot do it: the focus is on the list item.
pub fn connect_menu_key(view: &gtk::ListView) {
    let keys = gtk::EventControllerKey::new();
    let view_ref = view.downgrade();
    keys.connect_key_pressed(move |_, key, _, state| {
        let wants_menu =
            key == Key::Menu || (key == Key::F10 && state.contains(ModifierType::SHIFT_MASK));
        let opened = wants_menu
            && view_ref
                .upgrade()
                .is_some_and(|view| popup_focused_row(&view));
        if opened {
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
    });
    view.add_controller(keys);
}

/// Runs the menu action of the focused row. Returns `false` if no row has
/// the focus.
fn popup_focused_row(view: &gtk::ListView) -> bool {
    // The focus is on the list item. The row is its child.
    let row = view
        .root()
        .and_then(|root| root.focus())
        .and_then(|item| item.first_child());
    row.is_some_and(|row| {
        row.activate_action(&format!("{ROW_GROUP}.{POPUP_ACTION}"), None)
            .is_ok()
    })
}

fn text_label(css_classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(EllipsizeMode::End)
        .css_classes(css_classes)
        .build()
}

/// Opens the menu with a right click, a long press, and the row action.
fn connect_menu_triggers(root: &gtk::Box, menu: &gtk::PopoverMenu) {
    let click = gtk::GestureClick::builder().button(3).build();
    let at_point = menu.clone();
    click.connect_pressed(move |_, _, x, y| popup_at(&at_point, x, y));
    root.add_controller(click);

    let press = gtk::GestureLongPress::builder().touch_only(true).build();
    let at_point = menu.clone();
    press.connect_pressed(move |_, x, y| popup_at(&at_point, x, y));
    root.add_controller(press);

    let group = SimpleActionGroup::new();
    let action = SimpleAction::new(POPUP_ACTION, None);
    let whole_row = menu.clone();
    action.connect_activate(move |_, _| {
        whole_row.set_pointing_to(None::<&Rectangle>);
        whole_row.popup();
    });
    group.add_action(&action);
    root.insert_action_group(ROW_GROUP, Some(&group));

    // The popover is a child of the row. It must go before the row does.
    let menu = menu.clone();
    root.connect_destroy(move |_| menu.unparent());
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a pointer position fits in i32"
)]
fn popup_at(menu: &gtk::PopoverMenu, x: f64, y: f64) {
    menu.set_pointing_to(Some(&Rectangle::new(x as i32, y as i32, 1, 1)));
    menu.popup();
}

/// Builds the context menu of `item`. The go-to entries have no target when
/// the ID is not known. GTK then turns them off.
fn build_menu(item: &SongItem) -> Menu {
    let id = item.id().to_variant();
    let playback = Menu::new();
    playback.append_item(&entry(&gettext("Play"), PLAY_ITEM, Some(&id)));
    playback.append_item(&entry(&gettext("Play Next"), PLAY_NEXT, Some(&id)));
    playback.append_item(&entry(&gettext("Add to Queue"), ADD_TO_QUEUE, Some(&id)));
    let links = Menu::new();
    let album = item.album_id.as_deref().map(ToVariant::to_variant);
    let artist = item.artist_id.as_deref().map(ToVariant::to_variant);
    links.append_item(&entry(&gettext("Go to Album"), OPEN_ALBUM, album.as_ref()));
    links.append_item(&entry(
        &gettext("Go to Artist"),
        OPEN_ARTIST,
        artist.as_ref(),
    ));
    let menu = Menu::new();
    menu.append_section(None, &playback);
    menu.append_section(None, &links);
    menu
}

fn entry(label: &str, action: &str, target: Option<&Variant>) -> MenuItem {
    let item = MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action), target);
    item
}
