// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Library pages: playlists, albums, artists, and songs. One component serves
//! the four pages. Each page loads the next page of the list when the user
//! scrolls near the end.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use relm4::typed_view::list::TypedListView;
use ringo_am_api::endpoints;
use ringo_am_api::{Page, Result as ApiResult};
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::item::{SongItem, TileItem};
use crate::item_actions::{self, SongCatalog};
use crate::name_filter::NameFilter;
use crate::paged::{self, PageCursor};
use crate::section::Section;
use crate::services::{ApiClient, AppServices};
use crate::views::phase::Phase;
use crate::views::section_page;
use crate::widgets::library_tile::{ArtistRow, GridTile};
use crate::widgets::song_row::{self, SongRow};

/// The list that a library page shows.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LibraryKind {
    Playlists,
    Albums,
    Artists,
    Songs,
}

impl LibraryKind {
    /// All pages, in the order of the sidebar.
    pub const ALL: [Self; 4] = [Self::Playlists, Self::Albums, Self::Artists, Self::Songs];

    /// The sidebar section of the page.
    pub fn section(self) -> Section {
        match self {
            Self::Playlists => Section::Playlists,
            Self::Albums => Section::Albums,
            Self::Artists => Section::Artists,
            Self::Songs => Section::Songs,
        }
    }

    fn failed_title(self) -> String {
        match self {
            Self::Playlists => gettext("Cannot Load Playlists"),
            Self::Albums => gettext("Cannot Load Albums"),
            Self::Artists => gettext("Cannot Load Artists"),
            Self::Songs => gettext("Cannot Load Songs"),
        }
    }
}

/// One item of a library list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Entry {
    Tile(TileItem),
    Song(SongItem),
}

/// The list widget of a page, with the items that it shows.
enum Content {
    Tiles {
        view: TypedGridView<GridTile, gtk::SingleSelection>,
        items: Vec<TileItem>,
    },
    Artists {
        view: TypedListView<ArtistRow, gtk::SingleSelection>,
        items: Vec<TileItem>,
    },
    Songs {
        view: TypedListView<SongRow, gtk::SingleSelection>,
        items: Vec<SongItem>,
    },
}

impl Content {
    /// Makes an empty list for `kind`. A press on an item sends its position.
    fn new(kind: LibraryKind, filter: &NameFilter, sender: &ComponentSender<LibraryView>) -> Self {
        let input = sender.input_sender().clone();
        let activated = move |position| input.emit(LibraryInput::Activated(position));
        match kind {
            LibraryKind::Playlists | LibraryKind::Albums => {
                let mut view = TypedGridView::<GridTile, gtk::SingleSelection>::new();
                let filter = filter.clone();
                view.add_filter(move |tile| tile.matches(&filter));
                view.view.set_single_click_activate(true);
                view.view
                    .connect_activate(move |_, position| activated(position));
                Self::Tiles {
                    view,
                    items: Vec::new(),
                }
            }
            LibraryKind::Artists => {
                let mut view = TypedListView::<ArtistRow, gtk::SingleSelection>::new();
                let filter = filter.clone();
                view.add_filter(move |row| row.matches(&filter));
                view.view.set_single_click_activate(true);
                view.view
                    .connect_activate(move |_, position| activated(position));
                Self::Artists {
                    view,
                    items: Vec::new(),
                }
            }
            LibraryKind::Songs => {
                let mut view = TypedListView::<SongRow, gtk::SingleSelection>::new();
                let filter = filter.clone();
                view.add_filter(move |row| row.matches(&filter));
                view.view.set_show_separators(true);
                view.view.set_single_click_activate(true);
                view.view
                    .connect_activate(move |_, position| activated(position));
                song_row::connect_menu_key(&view.view);
                Self::Songs {
                    view,
                    items: Vec::new(),
                }
            }
        }
    }

    /// The list widget.
    fn widget(&self) -> gtk::Widget {
        match self {
            Self::Tiles { view, .. } => view.view.clone().upcast(),
            Self::Artists { view, .. } => view.view.clone().upcast(),
            Self::Songs { view, .. } => view.view.clone().upcast(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Tiles { items, .. } | Self::Artists { items, .. } => items.is_empty(),
            Self::Songs { items, .. } => items.is_empty(),
        }
    }

    /// Applies the changed term of the filter to the list.
    fn refilter(&self) {
        match self {
            Self::Tiles { view, .. } => view.notify_filter_changed(0),
            Self::Artists { view, .. } => view.notify_filter_changed(0),
            Self::Songs { view, .. } => view.notify_filter_changed(0),
        };
    }

    /// Number of items that the filter shows.
    fn visible_count(&self, filter: &NameFilter) -> usize {
        match self {
            Self::Tiles { items, .. } | Self::Artists { items, .. } => {
                items.iter().filter(|item| item.matches(filter)).count()
            }
            Self::Songs { items, .. } => items.iter().filter(|song| song.matches(filter)).count(),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Tiles { view, items } => {
                view.clear();
                items.clear();
            }
            Self::Artists { view, items } => {
                view.clear();
                items.clear();
            }
            Self::Songs { view, items } => {
                view.clear();
                items.clear();
            }
        }
    }

    /// Adds `entries` at the end. The songs go to `catalog`, so that their
    /// menu actions find them.
    fn append(&mut self, entries: Vec<Entry>, catalog: &SongCatalog) {
        for entry in entries {
            match (&mut *self, entry) {
                (Self::Tiles { view, items }, Entry::Tile(tile)) => {
                    view.append(GridTile::new(tile.clone()));
                    items.push(tile);
                }
                (Self::Artists { view, items }, Entry::Tile(tile)) => {
                    view.append(ArtistRow::new(tile.clone()));
                    items.push(tile);
                }
                (Self::Songs { view, items }, Entry::Song(song)) => {
                    catalog.remember(std::slice::from_ref(&song));
                    view.append(SongRow::new(song.clone()));
                    items.push(song);
                }
                (Self::Tiles { .. } | Self::Artists { .. }, Entry::Song(_))
                | (Self::Songs { .. }, Entry::Tile(_)) => {
                    warn!("a library entry does not fit the list");
                }
            }
        }
    }

    /// Runs the action of the shown item at `position`: it opens a tile, and
    /// it plays the shown songs from a song.
    fn activate(&self, position: u32, filter: &NameFilter, core: &CoreHandle) {
        let index = usize::try_from(position).unwrap_or(usize::MAX);
        match self {
            Self::Tiles { items, .. } | Self::Artists { items, .. } => {
                let item = items.iter().filter(|item| item.matches(filter)).nth(index);
                let Some(item) = item else {
                    return;
                };
                let target = item.id.to_variant();
                if let Err(error) = self
                    .widget()
                    .activate_action(item.kind.open_action(), Some(&target))
                {
                    warn!(%error, "cannot open the library item");
                }
            }
            Self::Songs { items, .. } => {
                let shown: Vec<SongItem> = items
                    .iter()
                    .filter(|song| song.matches(filter))
                    .cloned()
                    .collect();
                item_actions::play_songs(core, &shown, index);
            }
        }
    }
}

/// What a library page needs to start.
pub struct LibraryInit {
    pub kind: LibraryKind,
    pub services: AppServices,
    pub catalog: SongCatalog,
    /// The back button of the header bar goes to the sidebar of this view.
    pub split_view: adw::NavigationSplitView,
}

/// Input of a library page.
#[derive(Debug)]
pub enum LibraryInput {
    /// The page is on screen. The first call loads the first page.
    Shown,
    /// Loads the list again from the first page.
    Retry,
    /// The user scrolled near the end of the list.
    NearEnd,
    /// A load ended. The number tells which list it was for.
    Loaded(u64, Option<Page<Entry>>),
    /// The user activated the shown item at the position.
    Activated(u32),
    /// The user changed the text of the filter entry.
    FilterChanged(String),
}

/// A library page.
pub struct LibraryView {
    kind: LibraryKind,
    api: Arc<ApiClient>,
    core: CoreHandle,
    catalog: SongCatalog,
    phase: Phase,
    filter: NameFilter,
    cursor: PageCursor,
    pending: Option<JoinHandle<()>>,
    content: Content,
}

#[relm4::component(pub)]
impl SimpleComponent for LibraryView {
    type Init = LibraryInit;
    type Input = LibraryInput;
    type Output = ();

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                pack_start: &back_button,

                #[wrap(Some)]
                set_title_widget = &adw::Clamp {
                    set_maximum_size: 480,

                    #[wrap(Some)]
                    set_child = &filter_entry.clone() -> gtk::SearchEntry {},
                },
            },

            #[wrap(Some)]
            set_content = &gtk::Stack {
                #[watch]
                set_visible_child_name: model.phase.page(),

                add_named[Some(Phase::Loading.page())] = &adw::Spinner {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_width_request: 32,
                    set_height_request: 32,
                },

                add_named[Some(Phase::Empty.page())] = &adw::StatusPage {
                    set_icon_name: Some(model.kind.section().icon_name()),
                    set_title: &gettext("Your library is empty"),
                    set_description: Some(&gettext("Add music to your library to see it here.")),
                },

                add_named[Some(Phase::NoMatches.page())] = &adw::StatusPage {
                    set_icon_name: Some("edit-find-symbolic"),
                    set_title: &gettext("No Matches"),
                    set_description: Some(&gettext("Try a different name.")),
                },

                add_named[Some(Phase::Failed.page())] = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &model.kind.failed_title(),
                    set_description: Some(&gettext("Check your connection and try again.")),

                    #[wrap(Some)]
                    set_child = &gtk::Button {
                        set_label: &gettext("Try Again"),
                        set_halign: gtk::Align::Center,
                        add_css_class: "pill",
                        add_css_class: "suggested-action",
                        connect_clicked => LibraryInput::Retry,
                    },
                },

                add_named[Some(Phase::Content.page())] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    append = &scrolled.clone() -> gtk::ScrolledWindow {
                        set_vexpand: true,
                    },

                    // Tells the user that the next page loads.
                    append = &adw::Spinner {
                        set_halign: gtk::Align::Center,
                        set_width_request: 24,
                        set_height_request: 24,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        #[watch]
                        set_visible: model.cursor.is_loading() && model.phase == Phase::Content,
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let LibraryInit {
            kind,
            services,
            catalog,
            split_view,
        } = init;
        let back_button = section_page::back_button(&split_view);
        let filter = NameFilter::default();
        let content = Content::new(kind, &filter, &sender);
        let filter_entry = build_filter_entry(&sender);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&content.widget())
            .build();
        let input = sender.input_sender().clone();
        paged::connect_near_end(&scrolled, move || input.emit(LibraryInput::NearEnd));
        // The page can show again later. Only the first show loads.
        let input = sender.input_sender().clone();
        root.connect_map(move |_| input.emit(LibraryInput::Shown));

        let model = Self {
            kind,
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            phase: Phase::Idle,
            filter,
            cursor: PageCursor::default(),
            pending: None,
            content,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            LibraryInput::Shown => {
                if self.phase == Phase::Idle {
                    self.reload(&sender);
                }
            }
            LibraryInput::Retry => self.reload(&sender),
            LibraryInput::NearEnd => {
                if self.phase == Phase::Content {
                    self.load_next(&sender);
                }
            }
            LibraryInput::Loaded(generation, page) => self.finish(generation, page, &sender),
            LibraryInput::Activated(position) => {
                self.content.activate(position, &self.filter, &self.core);
            }
            LibraryInput::FilterChanged(term) => {
                self.filter.set(&term);
                self.content.refilter();
                self.refresh_phase();
                self.load_for_filter(&sender);
            }
        }
    }
}

impl LibraryView {
    /// Clears the list and loads it again from the first page.
    fn reload(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.cursor.reset();
        self.content.clear();
        self.phase = Phase::Loading;
        self.load_next(sender);
    }

    /// Requests the next page, if no load runs and a page is left.
    fn load_next(&mut self, sender: &ComponentSender<Self>) {
        let Some((generation, offset)) = self.cursor.begin() else {
            return;
        };
        let (api, kind) = (Arc::clone(&self.api), self.kind);
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let page = fetch(&api, kind, offset).await;
            input.emit(LibraryInput::Loaded(generation, page));
        }));
    }

    /// Shows the end of a load: the new items, or the failure. A failure of a
    /// later page keeps the items. The next scroll tries the page again.
    fn finish(
        &mut self,
        generation: u64,
        page: Option<Page<Entry>>,
        sender: &ComponentSender<Self>,
    ) {
        let Some(page) = page else {
            if self.cursor.fail(generation) && self.phase == Phase::Loading {
                self.phase = Phase::Failed;
            }
            return;
        };
        if !self.cursor.finish(generation, page.next_offset) {
            return;
        }
        self.content.append(page.items, &self.catalog);
        self.refresh_phase();
        self.load_for_filter(sender);
    }

    /// Sets the phase from the items, the filter, and the pages that are left.
    /// The phase stays `Loading` while the filter hides all loaded items and
    /// more pages can still bring a match.
    fn refresh_phase(&mut self) {
        if matches!(self.phase, Phase::Idle | Phase::Failed) {
            return;
        }
        self.phase = if self.content.is_empty() {
            Phase::Empty
        } else if self.content.visible_count(&self.filter) > 0 {
            Phase::Content
        } else if self.cursor.has_more() {
            Phase::Loading
        } else {
            Phase::NoMatches
        };
    }

    /// While a filter is set, loads all pages: a match can be on a page that
    /// is not loaded yet.
    fn load_for_filter(&mut self, sender: &ComponentSender<Self>) {
        let waiting = matches!(
            self.phase,
            Phase::Loading | Phase::Content | Phase::NoMatches
        );
        if self.filter.is_active() && waiting {
            self.load_next(sender);
        }
    }
}

/// Builds the entry that filters the list by name.
fn build_filter_entry(sender: &ComponentSender<LibraryView>) -> gtk::SearchEntry {
    let entry = gtk::SearchEntry::builder()
        .hexpand(true)
        .placeholder_text(gettext("Filter by name"))
        .build();
    let input = sender.input_sender().clone();
    entry.connect_search_changed(move |entry| {
        input.emit(LibraryInput::FilterChanged(entry.text().to_string()));
    });
    entry
}

/// Gets one page of the list of `kind`. Returns `None` if the request fails.
async fn fetch(api: &ApiClient, kind: LibraryKind, offset: u32) -> Option<Page<Entry>> {
    match request(api, kind, offset).await {
        Ok(page) => Some(page),
        Err(error) => {
            warn!(%error, ?kind, offset, "cannot load a library page");
            None
        }
    }
}

async fn request(api: &ApiClient, kind: LibraryKind, offset: u32) -> ApiResult<Page<Entry>> {
    Ok(match kind {
        LibraryKind::Playlists => convert(
            &endpoints::library_playlists(api, offset).await?,
            |playlist| Entry::Tile(TileItem::from(playlist)),
        ),
        LibraryKind::Albums => convert(&endpoints::library_albums(api, offset).await?, |album| {
            Entry::Tile(TileItem::from(album))
        }),
        LibraryKind::Artists => {
            convert(&endpoints::library_artists(api, offset).await?, |artist| {
                Entry::Tile(TileItem::from(artist))
            })
        }
        LibraryKind::Songs => convert(&endpoints::library_songs(api, offset).await?, |song| {
            Entry::Song(SongItem::from(song))
        }),
    })
}

/// Converts the items of `page` and keeps the offset of the next page.
fn convert<T>(page: &Page<T>, entry_of: impl Fn(&T) -> Entry) -> Page<Entry> {
    Page {
        items: page.items.iter().map(entry_of).collect(),
        next_offset: page.next_offset,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    use ringo_am_api::models::LibraryAlbum;

    fn album(id: &str) -> LibraryAlbum {
        serde_json::from_value(json!({
            "id": id,
            "type": "library-albums",
            "attributes": { "name": id, "artistName": "Artist" }
        }))
        .expect("the test album is valid")
    }

    #[test]
    fn convert_keeps_the_order_and_the_next_offset() {
        let page = Page {
            items: vec![album("l.1"), album("l.2")],
            next_offset: Some(25),
        };
        let converted = convert(&page, |album| Entry::Tile(TileItem::from(album)));
        let ids: Vec<String> = converted
            .items
            .iter()
            .map(|entry| match entry {
                Entry::Tile(tile) => tile.id.clone(),
                Entry::Song(song) => song.id().to_owned(),
            })
            .collect();
        assert_eq!(
            (ids, converted.next_offset),
            (vec!["l.1".to_owned(), "l.2".to_owned()], Some(25))
        );
    }

    #[test]
    fn library_kind_maps_to_its_section() {
        assert_eq!(LibraryKind::Songs.section(), Section::Songs);
        assert_eq!(LibraryKind::Playlists.section(), Section::Playlists);
    }
}
