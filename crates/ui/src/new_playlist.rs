// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The dialog that creates a library playlist, and the action `win.new-playlist` that opens it
//! (decisions L3 and L8).

use std::rc::Rc;
use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gio::ActionEntry;
use relm4::gtk::glib::{Variant, VariantTy};
use ringo_am_api::models::{LibraryPlaylist, LibraryPlaylistId};
use ringo_am_api::write::{self, Track};

use crate::add_to_playlist;
use crate::library_write;
use crate::services::AppServices;
use crate::views::library::LibraryInput;

/// Width of the dialog, in pixels.
const DIALOG_WIDTH: i32 = 400;

/// Name of the action that opens the dialog with no songs.
const NEW_PLAYLIST_ACTION: &str = "new-playlist";

/// Name of the action that opens the dialog with songs. Its parameter is the list of song IDs.
const NEW_PLAYLIST_WITH_SONGS_ACTION: &str = "new-playlist-with-songs";

/// Adds the window actions `win.new-playlist` and `win.new-playlist-with-songs`. After a playlist
/// is created, the actions mark the library playlists as out of date through `playlists` and open
/// the new playlist page.
pub fn register(
    window: &adw::ApplicationWindow,
    services: AppServices,
    playlists: relm4::Sender<LibraryInput>,
) {
    let empty = {
        let (services, playlists) = (services.clone(), playlists.clone());
        ActionEntry::builder(NEW_PLAYLIST_ACTION)
            .activate(move |window: &adw::ApplicationWindow, _, _| {
                present_dialog(window, &services, &playlists, Vec::new());
            })
            .build()
    };
    let with_songs = ActionEntry::builder(NEW_PLAYLIST_WITH_SONGS_ACTION)
        .parameter_type(Some(VariantTy::STRING_ARRAY))
        .activate(move |window: &adw::ApplicationWindow, _, parameter| {
            let song_ids = parameter
                .and_then(Variant::get::<Vec<String>>)
                .unwrap_or_default();
            present_dialog(
                window,
                &services,
                &playlists,
                add_to_playlist::tracks_of(&song_ids),
            );
        })
        .build();
    window.add_action_entries([empty, with_songs]);
}

/// Shows the dialog. `tracks` go in the new playlist.
fn present_dialog(
    window: &adw::ApplicationWindow,
    services: &AppServices,
    playlists: &relm4::Sender<LibraryInput>,
    tracks: Vec<Track>,
) {
    let (window_for_open, playlists) = (window.clone(), playlists.clone());
    NewPlaylistDialog::present(window, services, tracks, move |playlist| {
        let id = playlist.id;
        playlists.emit(LibraryInput::Invalidate);
        open_playlist(&window_for_open, &id);
    });
}

/// Opens the page of a library playlist with the window action.
fn open_playlist(window: &adw::ApplicationWindow, id: &LibraryPlaylistId) {
    if let Err(error) = gtk::prelude::WidgetExt::activate_action(
        window,
        "win.open-playlist",
        Some(&id.as_str().to_variant()),
    ) {
        tracing::warn!(%error, "cannot open the new playlist");
    }
}

/// Asks for a name and a description, then creates a library playlist.
#[derive(Clone)]
pub struct NewPlaylistDialog {
    dialog: adw::Dialog,
    content: adw::ToolbarView,
    name: adw::EntryRow,
    description: adw::EntryRow,
    create: gtk::Button,
}

impl NewPlaylistDialog {
    /// Shows the dialog over `parent`. `tracks` go in the new playlist, in order, and can be
    /// empty. `on_created` runs on the main thread after the request succeeds, and the dialog
    /// is closed then. A failed request keeps the dialog open.
    pub fn present(
        parent: &(impl IsA<gtk::Widget> + Clone),
        services: &AppServices,
        tracks: Vec<Track>,
        on_created: impl Fn(LibraryPlaylist) + 'static,
    ) {
        let this = Rc::new(Self::build());
        let api = Arc::clone(services.api());
        let on_created = Rc::new(on_created);
        let anchor = parent.clone();
        this.create.connect_clicked({
            let this = Rc::clone(&this);
            move |_| {
                this.submit(&anchor, &api, &tracks, &on_created);
            }
        });
        this.dialog.present(Some(parent));
        this.name.grab_focus();
    }

    fn build() -> Self {
        let name = adw::EntryRow::builder()
            .title(gettext("_Name"))
            .use_underline(true)
            .activates_default(true)
            .build();
        let description = adw::EntryRow::builder()
            .title(gettext("_Description"))
            .use_underline(true)
            .activates_default(true)
            .build();
        let group = adw::PreferencesGroup::new();
        group.add(&name);
        group.add(&description);
        let clamp = adw::Clamp::builder()
            .child(&group)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let cancel = gtk::Button::builder()
            .label(gettext("_Cancel"))
            .use_underline(true)
            .build();
        let create = gtk::Button::builder()
            .label(gettext("_Create"))
            .use_underline(true)
            .css_classes(["suggested-action"])
            .sensitive(false)
            .build();
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(false)
            .show_start_title_buttons(false)
            .build();
        header.pack_start(&cancel);
        header.pack_end(&create);
        let content = adw::ToolbarView::builder().content(&clamp).build();
        content.add_top_bar(&header);

        let dialog = adw::Dialog::builder()
            .title(gettext("New Playlist"))
            .content_width(DIALOG_WIDTH)
            .child(&content)
            .default_widget(&create)
            .focus_widget(&name)
            .build();
        cancel.connect_clicked({
            let dialog = dialog.clone();
            move |_| {
                dialog.close();
            }
        });
        name.connect_changed({
            let create = create.clone();
            move |name| create.set_sensitive(!name.text().trim().is_empty())
        });
        Self {
            dialog,
            content,
            name,
            description,
            create,
        }
    }

    /// Sends one create request and locks the dialog until it ends.
    fn submit(
        &self,
        anchor: &impl IsA<gtk::Widget>,
        api: &Arc<crate::services::ApiClient>,
        tracks: &[Track],
        on_created: &Rc<impl Fn(LibraryPlaylist) + 'static>,
    ) {
        let name = self.name.text().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let description = self.description.text().trim().to_owned();
        self.set_busy(true);
        let (api, tracks) = (Arc::clone(api), tracks.to_vec());
        let write = async move {
            let description = (!description.is_empty()).then_some(description);
            write::create_playlist(api.as_ref(), &name, description.as_deref(), &tracks).await
        };
        let on_created = Rc::clone(on_created);
        let (on_success_dialog, on_failure_dialog) = (self.clone(), self.clone());
        library_write::run(
            anchor,
            write,
            move |playlist| {
                on_success_dialog.dialog.force_close();
                on_created(playlist);
            },
            move || on_failure_dialog.set_busy(false),
        );
    }

    /// Locks or unlocks the dialog. The dialog cannot close while a request runs. The Create
    /// button is insensitive while locked, so a second click does nothing (rule R8). The name
    /// is not empty here, so the button is sensitive again after an unlock.
    fn set_busy(&self, busy: bool) {
        self.content.set_sensitive(!busy);
        self.dialog.set_can_close(!busy);
        self.create.set_sensitive(!busy);
    }
}
