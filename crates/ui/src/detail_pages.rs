// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The actions that open the album, artist, and playlist pages. A page pushes
//! on the navigation stack of the section that the user sees.

use std::any::Any;
use std::cell::RefCell;
use std::iter;
use std::rc::Rc;

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::glib::Variant;
use relm4::prelude::*;

use crate::detail::ArtistList;
use crate::item::TileKind;
use crate::item_actions::{self, SongCatalog};
use crate::services::AppServices;
use crate::views::album::{AlbumInit, AlbumView};
use crate::views::artist::{ArtistInit, ArtistView};
use crate::views::more::{MoreInit, MoreKind, MoreView};
use crate::views::playlist::{PlaylistInit, PlaylistInput, PlaylistView};
use crate::views::search::{Scope, SearchSection};

/// What the pages need to open.
pub struct PageSources {
    pub services: AppServices,
    pub catalog: SongCatalog,
    /// The split view of the window. Opening a page shows its content.
    pub split_view: adw::NavigationSplitView,
    /// The stack with one navigation view for each section.
    pub sections: gtk::Stack,
}

/// A page on a navigation stack, with the component that runs it. The
/// component stops when the page is removed.
struct OpenPage {
    page: adw::NavigationPage,
    _component: Box<dyn Any>,
    /// The ID of the playlist and the input of its page, if the page shows a playlist.
    playlist: Option<(String, relm4::Sender<PlaylistInput>)>,
}

/// Opens the detail pages. Clones share the list of open pages.
#[derive(Clone)]
struct Opener {
    sources: Rc<PageSources>,
    open_pages: Rc<RefCell<Vec<OpenPage>>>,
}

/// Adds the actions `win.open-album`, `win.open-artist`, and
/// `win.open-playlist` to `window`. Each takes the ID of the item. Also adds
/// `win.reload-playlist`, which loads the open pages of a playlist again, and
/// the actions that open all items of an artist list or of a search section.
pub fn register(window: &adw::ApplicationWindow, sources: PageSources) {
    let opener = Opener {
        sources: Rc::new(sources),
        open_pages: Rc::new(RefCell::new(Vec::new())),
    };
    opener.release_popped_pages();
    let kinds = [
        ("open-album", TileKind::Album),
        ("open-artist", TileKind::Artist),
        ("open-playlist", TileKind::Playlist),
    ];
    for (name, kind) in kinds {
        let action = item_actions::item_action(name);
        let opener = opener.clone();
        action.connect_activate(move |_, parameter| {
            if let Some(id) = parameter.and_then(Variant::str) {
                opener.open(kind, id);
            }
        });
        window.add_action(&action);
    }
    let reload = item_actions::item_action("reload-playlist");
    let reloader = opener.clone();
    reload.connect_activate(move |_, parameter| {
        if let Some(id) = parameter.and_then(Variant::str) {
            reloader.reload_playlist(id);
        }
    });
    window.add_action(&reload);
    let tile_pages = ArtistList::ALL.into_iter().map(|list| {
        (
            item_actions::open_artist_list_name(list),
            MoreKind::Tiles(list),
        )
    });
    let more_pages =
        iter::once(("open-artist-songs".to_owned(), MoreKind::TopSongs)).chain(tile_pages);
    for (name, kind) in more_pages {
        add_list_action(window, &name, &opener, move |opener, id, artist_name| {
            opener.open_more(kind, id, artist_name);
        });
    }
    for section in SearchSection::ALL {
        add_list_action(
            window,
            section.action_name(),
            &opener,
            move |opener, scope, term| {
                let kind = MoreKind::Search(Scope::from_name(&scope), section);
                opener.open_more(kind, term.clone(), term);
            },
        );
    }
}

/// Adds the action `name` to `window`. Its parameter is two strings, which
/// `open` gets.
fn add_list_action(
    window: &adw::ApplicationWindow,
    name: &str,
    opener: &Opener,
    open: impl Fn(&Opener, String, String) + 'static,
) {
    let action = item_actions::list_action(name);
    let opener = opener.clone();
    action.connect_activate(move |_, parameter| {
        if let Some((first, second)) = parameter.and_then(Variant::get::<(String, String)>) {
            open(&opener, first, second);
        }
    });
    window.add_action(&action);
}

impl Opener {
    /// The navigation view of the section that the user sees.
    fn active_navigation(&self) -> Option<adw::NavigationView> {
        self.sources
            .sections
            .visible_child()
            .and_downcast::<adw::NavigationView>()
    }

    /// Pushes the page of the item on the active navigation stack.
    fn open(&self, kind: TileKind, id: &str) {
        let Some(navigation) = self.active_navigation() else {
            return;
        };
        let open_page = self.launch(kind, id.to_owned());
        navigation.push(&open_page.page);
        self.open_pages.borrow_mut().push(open_page);
        self.sources.split_view.set_show_content(true);
    }

    /// Pushes the page with all items of one list. `subject` is the artist
    /// ID or the search term. `name` goes in the title.
    fn open_more(&self, kind: MoreKind, subject: String, name: String) {
        let Some(navigation) = self.active_navigation() else {
            return;
        };
        let view = MoreView::builder().launch(MoreInit {
            services: self.sources.services.clone(),
            catalog: self.sources.catalog.clone(),
            subject,
            name,
            kind,
        });
        let open_page = OpenPage {
            page: view.widget().clone(),
            _component: Box::new(view),
            playlist: None,
        };
        navigation.push(&open_page.page);
        self.open_pages.borrow_mut().push(open_page);
    }

    /// Starts the component of the page.
    fn launch(&self, kind: TileKind, id: String) -> OpenPage {
        let (services, catalog) = (self.sources.services.clone(), self.sources.catalog.clone());
        match kind {
            TileKind::Album => {
                let view = AlbumView::builder().launch(AlbumInit {
                    services,
                    catalog,
                    id,
                });
                OpenPage {
                    page: view.widget().clone(),
                    _component: Box::new(view),
                    playlist: None,
                }
            }
            TileKind::Artist => {
                let view = ArtistView::builder().launch(ArtistInit {
                    services,
                    catalog,
                    id,
                });
                OpenPage {
                    page: view.widget().clone(),
                    _component: Box::new(view),
                    playlist: None,
                }
            }
            TileKind::Playlist => {
                let view = PlaylistView::builder().launch(PlaylistInit {
                    services,
                    catalog,
                    id: id.clone(),
                });
                OpenPage {
                    page: view.widget().clone(),
                    playlist: Some((id, view.sender().clone())),
                    _component: Box::new(view),
                }
            }
        }
    }

    /// Loads the open pages of the library playlist `id` again.
    fn reload_playlist(&self, id: &str) {
        for open in self.open_pages.borrow().iter() {
            if let Some((page_id, input)) = &open.playlist
                && page_id == id
            {
                input.emit(PlaylistInput::Retry);
            }
        }
    }

    /// Stops the component of a page when the user goes back from it.
    fn release_popped_pages(&self) {
        let mut child = self.sources.sections.first_child();
        while let Some(widget) = child {
            if let Some(navigation) = widget.downcast_ref::<adw::NavigationView>() {
                let open_pages = Rc::clone(&self.open_pages);
                navigation.connect_popped(move |_, popped| {
                    open_pages.borrow_mut().retain(|open| &open.page != popped);
                });
            }
            child = widget.next_sibling();
        }
    }
}
