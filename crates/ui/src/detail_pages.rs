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
use crate::views::artist_more::{ArtistMoreInit, ArtistMoreView, MoreKind};
use crate::views::playlist::{PlaylistInit, PlaylistView};

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
}

/// Opens the detail pages. Clones share the list of open pages.
#[derive(Clone)]
struct Opener {
    sources: Rc<PageSources>,
    open_pages: Rc<RefCell<Vec<OpenPage>>>,
}

/// Adds the actions `win.open-album`, `win.open-artist`, and
/// `win.open-playlist` to `window`. Each takes the ID of the item.
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
    let tile_pages = ArtistList::ALL.into_iter().map(|list| {
        (
            item_actions::open_artist_list_name(list),
            MoreKind::Tiles(list),
        )
    });
    let more_pages =
        iter::once(("open-artist-songs".to_owned(), MoreKind::TopSongs)).chain(tile_pages);
    for (name, kind) in more_pages {
        let action = item_actions::artist_list_action(&name);
        let opener = opener.clone();
        action.connect_activate(move |_, parameter| {
            if let Some((id, artist_name)) = parameter.and_then(Variant::get::<(String, String)>) {
                opener.open_more(kind, id, artist_name);
            }
        });
        window.add_action(&action);
    }
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

    /// Pushes the page with all top songs or all tiles of one list of an
    /// artist.
    fn open_more(&self, kind: MoreKind, id: String, artist_name: String) {
        let Some(navigation) = self.active_navigation() else {
            return;
        };
        let view = ArtistMoreView::builder().launch(ArtistMoreInit {
            services: self.sources.services.clone(),
            catalog: self.sources.catalog.clone(),
            id,
            artist_name,
            kind,
        });
        let open_page = OpenPage {
            page: view.widget().clone(),
            _component: Box::new(view),
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
                }
            }
            TileKind::Playlist => {
                let view = PlaylistView::builder().launch(PlaylistInit {
                    services,
                    catalog,
                    id,
                });
                OpenPage {
                    page: view.widget().clone(),
                    _component: Box::new(view),
                }
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
