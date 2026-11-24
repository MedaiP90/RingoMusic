// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Catalog search: a search entry in the header bar and the results in four
//! sections.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use ringo_am_api::endpoints::{self, CatalogSearchType};
use ringo_am_api::models::{ApiPage, SearchResults};
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::detail::storefront;
use crate::item::{SongItem, TileItem};
use crate::item_actions::{self, SongCatalog};
use crate::services::{ApiClient, AppServices};
use crate::widgets::song_carousel::SongCarousel;
use crate::widgets::tile::{self, TileRow};

/// Time that the entry waits after the last key before it searches.
const SEARCH_DELAY_MS: u32 = 300;

/// Maximum number of results for each type. Apple allows at most 25.
const RESULT_LIMIT: u32 = 25;

const SEARCH_TYPES: [CatalogSearchType; 4] = [
    CatalogSearchType::Songs,
    CatalogSearchType::Albums,
    CatalogSearchType::Artists,
    CatalogSearchType::Playlists,
];

/// The results of one search, ready for the widgets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchSections {
    songs: Vec<SongItem>,
    albums: Vec<TileItem>,
    artists: Vec<TileItem>,
    playlists: Vec<TileItem>,
}

impl SearchSections {
    fn is_empty(&self) -> bool {
        self.songs.is_empty()
            && self.albums.is_empty()
            && self.artists.is_empty()
            && self.playlists.is_empty()
    }
}

impl From<&SearchResults> for SearchSections {
    fn from(results: &SearchResults) -> Self {
        Self {
            songs: convert(results.songs.as_ref()),
            albums: convert(results.albums.as_ref()),
            artists: convert(results.artists.as_ref()),
            playlists: convert(results.playlists.as_ref()),
        }
    }
}

fn convert<T, U: for<'a> From<&'a T>>(page: Option<&ApiPage<T>>) -> Vec<U> {
    page.map_or_else(Vec::new, |page| page.data.iter().map(U::from).collect())
}

/// What the view shows.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Phase {
    /// No term yet.
    Hint,
    Loading,
    Empty,
    Failed,
    Results,
}

impl Phase {
    /// Name of the stack page.
    fn page(self) -> &'static str {
        match self {
            Self::Hint => "hint",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Failed => "failed",
            Self::Results => "results",
        }
    }
}

/// What the search view needs to start.
pub struct SearchInit {
    pub services: AppServices,
    pub catalog: SongCatalog,
}

/// Input of the search view.
#[derive(Debug)]
pub enum SearchInput {
    /// The user typed a term. An empty term clears the results.
    TermChanged(String),
    /// Searches the last term again.
    Retry,
    /// A search ended. The number tells which search it was.
    Loaded(u64, Option<SearchSections>),
    /// The user activated the song at the position.
    SongActivated(usize),
    /// Puts the cursor in the search entry.
    Focus,
}

/// The search page.
pub struct SearchView {
    api: Arc<ApiClient>,
    core: CoreHandle,
    catalog: SongCatalog,
    entry: gtk::SearchEntry,
    phase: Phase,
    term: String,
    /// Number of the last search. The result of an older search is dropped.
    generation: u64,
    pending: Option<JoinHandle<()>>,
    songs: SongCarousel,
    song_items: Vec<SongItem>,
    songs_section: gtk::Box,
    albums: TileRow,
    artists: TileRow,
    playlists: TileRow,
}

#[relm4::component(pub)]
impl SimpleComponent for SearchView {
    type Init = SearchInit;
    type Input = SearchInput;
    type Output = ();

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &adw::Clamp {
                    set_maximum_size: 480,

                    #[wrap(Some)]
                    set_child = &search_entry.clone() -> gtk::SearchEntry {},
                },
            },

            #[wrap(Some)]
            set_content = &gtk::Stack {
                #[watch]
                set_visible_child_name: model.phase.page(),

                add_named[Some(Phase::Hint.page())] = &adw::StatusPage {
                    set_icon_name: Some("system-search-symbolic"),
                    set_title: &gettext("Search Apple Music"),
                    set_description: Some(&gettext("Look for songs, albums, artists, and playlists.")),
                },

                add_named[Some(Phase::Loading.page())] = &adw::Spinner {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_width_request: 32,
                    set_height_request: 32,
                },

                add_named[Some(Phase::Empty.page())] = &adw::StatusPage {
                    set_icon_name: Some("edit-find-symbolic"),
                    set_title: &gettext("No Results"),
                    set_description: Some(&gettext("Try a different term.")),
                },

                add_named[Some(Phase::Failed.page())] = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &gettext("Search Failed"),
                    set_description: Some(&gettext("Check your connection and try again.")),

                    #[wrap(Some)]
                    set_child = &gtk::Button {
                        set_label: &gettext("Try Again"),
                        set_halign: gtk::Align::Center,
                        add_css_class: "pill",
                        add_css_class: "suggested-action",
                        connect_clicked => SearchInput::Retry,
                    },
                },

                add_named[Some(Phase::Results.page())] = &results_page.clone() -> gtk::ScrolledWindow {},
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let SearchInit { services, catalog } = init;
        let search_entry = build_entry(&sender);
        let input = sender.input_sender().clone();
        let songs =
            SongCarousel::new(move |position| input.emit(SearchInput::SongActivated(position)));
        let songs_section = build_songs_section(&songs);
        let (albums, artists, playlists) = (
            TileRow::new(&gettext("Albums")),
            TileRow::new(&gettext("Artists")),
            TileRow::new(&gettext("Playlists")),
        );
        let results_page = build_results_page(&songs_section, [&albums, &artists, &playlists]);

        let model = Self {
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            entry: search_entry.clone(),
            phase: Phase::Hint,
            term: String::new(),
            generation: 0,
            pending: None,
            songs,
            song_items: Vec::new(),
            songs_section,
            albums,
            artists,
            playlists,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SearchInput::TermChanged(term) => self.search(&term, &sender),
            SearchInput::Retry => {
                let term = self.term.clone();
                self.search(&term, &sender);
            }
            SearchInput::Loaded(generation, sections) => {
                if generation == self.generation {
                    self.finish(sections);
                }
            }
            SearchInput::SongActivated(position) => {
                item_actions::play_songs(&self.core, &self.song_items, position);
            }
            SearchInput::Focus => {
                self.entry.grab_focus();
            }
        }
    }
}

impl SearchView {
    /// Starts a search for `term` and cancels the one that runs.
    fn search(&mut self, term: &str, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.generation += 1;
        term.trim().clone_into(&mut self.term);
        if self.term.is_empty() {
            self.phase = Phase::Hint;
            self.show(SearchSections::default());
            return;
        }
        let Some(storefront) = storefront(&self.core.subscribe().borrow()) else {
            self.phase = Phase::Failed;
            return;
        };
        self.phase = Phase::Loading;
        let (api, term, generation) = (Arc::clone(&self.api), self.term.clone(), self.generation);
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let sections = fetch(&api, &storefront, &term).await;
            input.emit(SearchInput::Loaded(generation, sections));
        }));
    }

    /// Shows the end of a search: the results, or the failure.
    fn finish(&mut self, sections: Option<SearchSections>) {
        let Some(sections) = sections else {
            self.phase = Phase::Failed;
            return;
        };
        self.phase = if sections.is_empty() {
            Phase::Empty
        } else {
            Phase::Results
        };
        self.show(sections);
    }

    /// Puts `sections` in the widgets. An empty section hides itself.
    fn show(&mut self, sections: SearchSections) {
        self.catalog.remember(&sections.songs);
        self.songs.set_songs(&sections.songs);
        self.songs_section.set_visible(!sections.songs.is_empty());
        self.albums.set_items(&sections.albums);
        self.artists.set_items(&sections.artists);
        self.playlists.set_items(&sections.playlists);
        self.song_items = sections.songs;
    }
}

/// Searches the catalog. Returns `None` if the request fails.
async fn fetch(api: &ApiClient, storefront: &str, term: &str) -> Option<SearchSections> {
    match endpoints::search(api, storefront, term, &SEARCH_TYPES, RESULT_LIMIT).await {
        Ok(results) => Some(SearchSections::from(&results)),
        Err(error) => {
            warn!(%error, "catalog search failed");
            None
        }
    }
}

/// Builds the entry. It sends the term after a short pause, and at once on Enter.
fn build_entry(sender: &ComponentSender<SearchView>) -> gtk::SearchEntry {
    let entry = gtk::SearchEntry::builder()
        .hexpand(true)
        .search_delay(SEARCH_DELAY_MS)
        .placeholder_text(gettext("Search Apple Music"))
        .build();
    let input = sender.input_sender().clone();
    entry.connect_search_changed(move |entry| {
        input.emit(SearchInput::TermChanged(entry.text().to_string()));
    });
    let input = sender.input_sender().clone();
    entry.connect_activate(move |entry| {
        input.emit(SearchInput::TermChanged(entry.text().to_string()));
    });
    entry.connect_stop_search(|entry| entry.set_text(""));
    entry
}

/// Builds the songs heading with the list under it. It hides itself when empty.
fn build_songs_section(carousel: &SongCarousel) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .visible(false)
        .build();
    section.append(&tile::heading(&gettext("Songs")));
    section.append(carousel.widget());
    section
}

/// Builds the scrolled page with the songs and the rows of tiles.
fn build_results_page(songs: &gtk::Box, rows: [&TileRow; 3]) -> gtk::ScrolledWindow {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(12)
        .margin_bottom(12)
        .build();
    content.append(songs);
    for row in rows {
        content.append(row.widget());
    }
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn results(value: serde_json::Value) -> SearchResults {
        serde_json::from_value(value).expect("the test results are valid")
    }

    fn song_json(id: &str) -> serde_json::Value {
        json!({ "id": id, "type": "songs", "attributes": { "name": id } })
    }

    #[test]
    fn sections_of_results_without_data_are_empty() {
        let sections = SearchSections::from(&results(json!({})));
        assert!(sections.is_empty());
    }

    #[test]
    fn sections_keep_the_order_of_the_songs() {
        let value = json!({ "songs": { "data": [song_json("1"), song_json("2")] } });
        let sections = SearchSections::from(&results(value));
        let ids: Vec<&str> = sections.songs.iter().map(SongItem::id).collect();
        assert_eq!(ids, ["1", "2"]);
        assert!(!sections.is_empty());
    }

    #[test]
    fn sections_convert_albums_artists_and_playlists() {
        let value = json!({
            "albums": { "data": [{ "id": "a", "type": "albums", "attributes": { "name": "A" } }] },
            "artists": { "data": [{ "id": "b", "type": "artists", "attributes": { "name": "B" } }] },
            "playlists": { "data": [{ "id": "c", "type": "playlists", "attributes": { "name": "C" } }] }
        });
        let sections = SearchSections::from(&results(value));
        assert_eq!(
            (
                sections.albums.len(),
                sections.artists.len(),
                sections.playlists.len()
            ),
            (1, 1, 1)
        );
    }
}
