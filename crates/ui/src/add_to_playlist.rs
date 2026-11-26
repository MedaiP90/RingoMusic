// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The "Add to Playlist" popover and the menu button that opens it (decisions L8 and L9).
//!
//! The popover lists the library playlists that the user can edit. It loads them each time it
//! opens, because `/v1/me/` is never cached (decision L5).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::gdk::Rectangle;
use relm4::gtk::gio::{Menu, MenuItem, SimpleAction, SimpleActionGroup};
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use relm4::typed_view::list::{RelmListItem, TypedListView};
use ringo_am_api::Page;
use ringo_am_api::endpoints;
use ringo_am_api::models::{LibraryPlaylist, LibraryPlaylistId};
use ringo_am_api::write::{self, CatalogItem, Track};
use tracing::warn;

use crate::add_to_library;
use crate::detail::is_library_id;
use crate::library_write;
use crate::name_filter::NameFilter;
use crate::services::{ApiClient, AppServices};

/// Width of the popover, in pixels.
const POPOVER_WIDTH: i32 = 300;

/// Height of the list area, in pixels.
const LIST_HEIGHT: i32 = 240;

/// Space inside the popover, in pixels.
const MARGIN: i32 = 6;

/// Most pages of library playlists that the popover loads. One page holds up to 100 playlists.
// ponytail: a user with more playlists sees the first ones. Load on scroll if this is too small.
const PAGES_LIMIT: usize = 10;

/// Names of the stack pages.
const PAGE_LOADING: &str = "loading";
const PAGE_CONTENT: &str = "content";
const PAGE_MESSAGE: &str = "message";

/// Name of the window action that opens the dialog of a new playlist with songs.
const NEW_PLAYLIST_ACTION: &str = "win.new-playlist-with-songs";

/// Name of the window action that loads an open playlist page again.
pub const RELOAD_PLAYLIST_ACTION: &str = "win.reload-playlist";

/// Name of the action group of the menu button, and of its actions.
const MENU_GROUP: &str = "item-menu";
const MENU_ACTION: &str = "add";
const LIBRARY_ACTION: &str = "add-to-library";

thread_local! {
    // A song row is set up without any context, so the popover gets the services here.
    static SERVICES: RefCell<Option<AppServices>> = const { RefCell::new(None) };
}

/// Sets the services that [`present`] and the library items use. Call it once, at start-up, on
/// the main thread.
pub fn install(services: AppServices) {
    SERVICES.with_borrow_mut(|installed| *installed = Some(services));
}

/// The services that [`install`] set. `None` before the start-up ends.
pub fn services() -> Option<AppServices> {
    SERVICES.with_borrow(Clone::clone)
}

/// Makes the tracks of songs. A library ID gives a library song. Any other ID gives a catalog
/// song. The order of `song_ids` stays.
pub fn tracks_of(song_ids: &[String]) -> Vec<Track> {
    song_ids
        .iter()
        .map(|id| {
            if is_library_id(id) {
                Track::LibrarySong(id.clone().into())
            } else {
                Track::Song(id.clone().into())
            }
        })
        .collect()
}

/// A library playlist in the list of the popover.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PlaylistRow {
    id: LibraryPlaylistId,
    name: String,
}

impl PlaylistRow {
    fn matches(&self, filter: &NameFilter) -> bool {
        filter.matches(&[&self.name])
    }
}

impl RelmListItem for PlaylistRow {
    type Root = gtk::Label;
    type Widgets = ();

    fn setup(_list_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        let label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .margin_top(MARGIN)
            .margin_bottom(MARGIN)
            .margin_start(MARGIN)
            .margin_end(MARGIN)
            .build();
        (label, ())
    }

    fn bind(&mut self, _widgets: &mut Self::Widgets, root: &mut Self::Root) {
        root.set_label(&self.name);
        root.set_tooltip_text(Some(&self.name));
    }
}

/// Keeps the playlists that the user can edit. Apple refuses a write to the others.
fn editable_rows(playlists: Vec<LibraryPlaylist>) -> Vec<PlaylistRow> {
    playlists
        .into_iter()
        .filter(|playlist| playlist.attributes.can_edit)
        .map(|playlist| PlaylistRow {
            id: playlist.id,
            name: playlist.attributes.name,
        })
        .collect()
}

/// Loads the library playlists that the user can edit.
async fn load_rows(api: &ApiClient) -> ringo_am_api::Result<Vec<PlaylistRow>> {
    let mut rows = Vec::new();
    let mut offset = 0;
    for _ in 0..PAGES_LIMIT {
        let Page { items, next_offset } = endpoints::library_playlists(api, offset).await?;
        rows.extend(editable_rows(items));
        match next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    Ok(rows)
}

/// The content of the popover: the search entry, the new playlist button, and the list.
struct Picker {
    stack: gtk::Stack,
    message: gtk::Label,
    list: RefCell<TypedListView<PlaylistRow, gtk::SingleSelection>>,
    filter: NameFilter,
}

impl Picker {
    fn new() -> Self {
        let filter = NameFilter::default();
        let mut list = TypedListView::<PlaylistRow, gtk::SingleSelection>::new();
        let row_filter = filter.clone();
        list.add_filter(move |row| row.matches(&row_filter));
        list.view.set_single_click_activate(true);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list.view)
            .build();
        let message = gtk::Label::builder()
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let stack = gtk::Stack::builder().height_request(LIST_HEIGHT).build();
        stack.add_named(&adw::Spinner::new(), Some(PAGE_LOADING));
        stack.add_named(&scrolled, Some(PAGE_CONTENT));
        stack.add_named(&message, Some(PAGE_MESSAGE));
        stack.set_visible_child_name(PAGE_LOADING);
        Self {
            stack,
            message,
            list: RefCell::new(list),
            filter,
        }
    }

    fn show_rows(&self, rows: Vec<PlaylistRow>) {
        if rows.is_empty() {
            self.show_message(&gettext("You have no playlist that you can add songs to."));
            return;
        }
        self.list.borrow_mut().extend_from_iter(rows);
        self.stack.set_visible_child_name(PAGE_CONTENT);
    }

    fn show_message(&self, text: &str) {
        self.message.set_label(text);
        self.stack.set_visible_child_name(PAGE_MESSAGE);
    }

    fn set_term(&self, term: &str) {
        self.filter.set(term);
        self.list.borrow().notify_filter_changed(0);
    }

    /// The playlist that the list shows at `position`.
    fn row_at(&self, position: u32) -> Option<PlaylistRow> {
        let list = self.list.borrow();
        let item = list.get_visible(position)?;
        let row = item.borrow().clone();
        Some(row)
    }
}

/// Opens the popover over `anchor`. `song_ids` are the songs to add, in order. The popover points
/// to `pointing_to`, or to the whole anchor if it is `None`.
///
/// The popover loads the playlists, so each opening shows the current list.
pub fn present(
    anchor: &impl IsA<gtk::Widget>,
    pointing_to: Option<&Rectangle>,
    song_ids: Vec<String>,
) {
    let Some(services) = services() else {
        warn!("the add to playlist popover has no services");
        return;
    };
    if song_ids.is_empty() {
        return;
    }
    let anchor: gtk::Widget = anchor.as_ref().clone();
    let picker = Rc::new(Picker::new());
    let popover = build_popover(&anchor, &picker, &song_ids);
    popover.set_pointing_to(pointing_to);
    connect_pick(&anchor, &picker, &popover, &services, song_ids);
    load(&picker, &services);
    // A menu can still be closing now. The popover opens after it.
    glib::idle_add_local_once(move || popover.popup());
}

/// Builds the popover and puts it on `anchor`. It leaves the tree when it closes.
fn build_popover(anchor: &gtk::Widget, picker: &Rc<Picker>, song_ids: &[String]) -> gtk::Popover {
    let entry = gtk::SearchEntry::builder()
        .placeholder_text(gettext("Search playlists"))
        .build();
    let search = Rc::clone(picker);
    entry.connect_search_changed(move |entry| search.set_term(&entry.text()));

    let new_playlist = gtk::Button::builder()
        .child(
            &adw::ButtonContent::builder()
                .label(gettext("New Playlist…"))
                .icon_name("list-add-symbolic")
                .halign(gtk::Align::Start)
                .build(),
        )
        .css_classes(["flat"])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(MARGIN)
        .width_request(POPOVER_WIDTH)
        .build();
    content.append(&entry);
    content.append(&new_playlist);
    content.append(&picker.stack);

    let popover = gtk::Popover::builder().child(&content).build();
    popover.set_parent(anchor);
    popover.connect_closed(|popover| {
        let popover = popover.clone();
        glib::idle_add_local_once(move || popover.unparent());
    });
    popover.connect_map({
        let entry = entry.clone();
        move |_| {
            entry.grab_focus();
        }
    });

    let (anchor, song_ids) = (anchor.clone(), song_ids.to_vec());
    let weak_popover = popover.downgrade();
    new_playlist.connect_clicked(move |_| {
        if let Some(popover) = weak_popover.upgrade() {
            popover.popdown();
        }
        if let Err(error) =
            anchor.activate_action(NEW_PLAYLIST_ACTION, Some(&song_ids.to_variant()))
        {
            warn!(%error, "cannot open the new playlist dialog");
        }
    });
    popover
}

/// Makes a press on a playlist add the songs to it.
fn connect_pick(
    anchor: &gtk::Widget,
    picker: &Rc<Picker>,
    popover: &gtk::Popover,
    services: &AppServices,
    song_ids: Vec<String>,
) {
    let (anchor, services) = (anchor.clone(), services.clone());
    let weak_picker = Rc::downgrade(picker);
    let weak_popover = popover.downgrade();
    picker
        .list
        .borrow()
        .view
        .connect_activate(move |_, position| {
            let Some(row) = weak_picker
                .upgrade()
                .and_then(|picker| picker.row_at(position))
            else {
                return;
            };
            if let Some(popover) = weak_popover.upgrade() {
                popover.popdown();
            }
            add_to(&anchor, &services, row, &song_ids);
        });
}

/// Sends one request that adds all songs to the playlist. On success, a toast tells it and an
/// open page of the playlist loads again.
fn add_to(anchor: &gtk::Widget, services: &AppServices, row: PlaylistRow, song_ids: &[String]) {
    let api = Arc::clone(services.api());
    let tracks = tracks_of(song_ids);
    let PlaylistRow { id, name } = row;
    let request_id = id.clone();
    let request = async move { write::add_to_playlist(api.as_ref(), &request_id, &tracks).await };
    let toast_anchor = anchor.clone();
    library_write::run(
        anchor,
        request,
        move |()| {
            let text = gettext("Added to {name}").replace("{name}", &name);
            library_write::notify(&toast_anchor, &text);
            if let Err(error) = toast_anchor
                .activate_action(RELOAD_PLAYLIST_ACTION, Some(&id.as_str().to_variant()))
            {
                warn!(%error, "cannot reload the playlist page");
            }
        },
        || {},
    );
}

/// Loads the playlists into the popover. It stops if the popover is gone.
fn load(picker: &Rc<Picker>, services: &AppServices) {
    let api = Arc::clone(services.api());
    let task = relm4::spawn(async move { load_rows(api.as_ref()).await });
    let weak_picker = Rc::downgrade(picker);
    glib::spawn_future_local(async move {
        let result = task.await;
        let Some(picker) = weak_picker.upgrade() else {
            return;
        };
        match result {
            Ok(Ok(rows)) => picker.show_rows(rows),
            Ok(Err(error)) => {
                warn!(%error, "cannot load the library playlists");
                picker.show_message(&gettext("Could not load your playlists."));
            }
            Err(join_error) => {
                warn!(%join_error, "the playlist load task did not finish");
                picker.show_message(&gettext("Could not load your playlists."));
            }
        }
    });
}

/// A menu button for a header or a bar. It has the items "Add to Playlist…" (set the songs with
/// [`Self::set_songs`]) and "Add to Library" (set the item with [`Self::set_library_item`]).
/// An item is hidden while it is off. The button is hidden while both are off.
pub struct ItemMenu {
    button: gtk::MenuButton,
    action: SimpleAction,
    song_ids: Rc<RefCell<Vec<String>>>,
    library_action: SimpleAction,
    library_item: Rc<RefCell<Option<CatalogItem>>>,
    /// Counts the calls of [`Self::set_library_item`]. A status result of an older call is dropped.
    library_generation: Rc<Cell<u64>>,
}

impl ItemMenu {
    /// Makes the button. It has no songs and no library item yet, so it is hidden.
    pub fn new() -> Self {
        let menu = Menu::new();
        menu.append_item(&hidden_when_off(&gettext("Add to Playlist…"), MENU_ACTION));
        menu.append_item(&hidden_when_off(&gettext("Add to Library"), LIBRARY_ACTION));
        let button = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(gettext("More"))
            .menu_model(&menu)
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .visible(false)
            .build();

        let song_ids = Rc::new(RefCell::new(Vec::new()));
        let action = SimpleAction::new(MENU_ACTION, None);
        action.set_enabled(false);
        let (weak_button, ids) = (button.downgrade(), Rc::clone(&song_ids));
        action.connect_activate(move |_, _| {
            if let Some(button) = weak_button.upgrade() {
                present(&button, None, ids.borrow().clone());
            }
        });

        let library_item = Rc::new(RefCell::new(None));
        let library_action = SimpleAction::new(LIBRARY_ACTION, None);
        library_action.set_enabled(false);
        let (weak_button, item) = (button.downgrade(), Rc::clone(&library_item));
        library_action.connect_activate(move |_, _| {
            let item: Option<CatalogItem> = item.borrow().clone();
            if let (Some(button), Some(item)) = (weak_button.upgrade(), item) {
                add_to_library::add(&button, item);
            }
        });

        let group = SimpleActionGroup::new();
        group.add_action(&action);
        group.add_action(&library_action);
        button.insert_action_group(MENU_GROUP, Some(&group));
        Self {
            button,
            action,
            song_ids,
            library_action,
            library_item,
            library_generation: Rc::new(Cell::new(0)),
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::MenuButton {
        &self.button
    }

    /// Sets the songs that the item adds, in order.
    pub fn set_songs(&self, song_ids: Vec<String>) {
        self.action.set_enabled(!song_ids.is_empty());
        *self.song_ids.borrow_mut() = song_ids;
        self.refresh_button();
    }

    /// Sets the catalog item that "Add to Library" adds. `None` hides the item, for an item of
    /// the library. The item stays hidden while a live request reads if the item is already in
    /// the library, and stays hidden if it is. After a failed request the item shows (decision
    /// L17). The result of an older call, or of a page that is gone, is dropped.
    pub fn set_library_item(&self, item: Option<CatalogItem>) {
        let generation = self.library_generation.get() + 1;
        self.library_generation.set(generation);
        self.library_action.set_enabled(false);
        self.refresh_button();
        self.library_item.borrow_mut().clone_from(&item);
        let Some(item) = item else {
            return;
        };
        let (button, action) = (self.button.downgrade(), self.library_action.clone());
        let (current, other) = (Rc::clone(&self.library_generation), self.action.clone());
        glib::spawn_future_local(async move {
            let in_library = add_to_library::is_in_library(item).await;
            let Some(button) = button.upgrade() else {
                return;
            };
            if current.get() != generation {
                return;
            }
            action.set_enabled(!in_library);
            button.set_visible(action.is_enabled() || other.is_enabled());
        });
    }

    /// Sets the song of a player bar, or `None` for no song. A catalog song shows both items. The
    /// library item shows with no status request, like a song row (decision L17): a request for
    /// each track change would be too many.
    pub fn set_song(&self, song_id: Option<String>) {
        self.set_songs(song_id.iter().cloned().collect());
        let item = song_id
            .filter(|id| !is_library_id(id))
            .map(|id| CatalogItem::Song(id.into()));
        // A status request of an earlier call must not change the item now.
        self.library_generation
            .set(self.library_generation.get() + 1);
        self.library_action.set_enabled(item.is_some());
        *self.library_item.borrow_mut() = item;
        self.refresh_button();
    }

    /// Shows the button while one of its items shows.
    fn refresh_button(&self) {
        self.button
            .set_visible(self.action.is_enabled() || self.library_action.is_enabled());
    }
}

/// An item of the menu button. It is hidden while its action is off.
fn hidden_when_off(label: &str, action: &str) -> MenuItem {
    let item = MenuItem::new(Some(label), Some(&format!("{MENU_GROUP}.{action}")));
    item.set_attribute_value("hidden-when", Some(&"action-disabled".to_variant()));
    item
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn playlist(id: &str, name: &str, can_edit: bool) -> LibraryPlaylist {
        serde_json::from_value(json!({
            "id": id,
            "attributes": { "name": name, "canEdit": can_edit }
        }))
        .expect("the playlist JSON is valid")
    }

    #[test]
    fn tracks_of_keeps_the_order_and_the_kind_of_each_id() {
        let ids = ["300", "i.AAA", "100"].map(str::to_owned);
        assert_eq!(
            tracks_of(&ids),
            vec![
                Track::Song("300".to_owned().into()),
                Track::LibrarySong("i.AAA".to_owned().into()),
                Track::Song("100".to_owned().into()),
            ]
        );
    }

    #[test]
    fn tracks_of_gives_nothing_for_no_songs() {
        assert_eq!(tracks_of(&[]), Vec::new());
    }

    #[test]
    fn editable_rows_drops_the_playlists_that_the_user_cannot_edit() {
        let rows = editable_rows(vec![
            playlist("p.1", "Mine", true),
            playlist("p.2", "Shared", false),
            playlist("p.3", "Other", true),
        ]);
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["Mine", "Other"]);
    }
}
