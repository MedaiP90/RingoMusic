// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Search of the catalog or of the library: a search entry and a scope
//! switch in the header bar, and the results in four sections. The Show More
//! button of a section opens a page with all its results.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use ringo_am_api::endpoints::{self, CatalogSearchType, LibrarySearchType};
use ringo_am_api::models::{ApiPage, LibrarySearchResults, SearchResults};
use ringo_am_api::{Page, Result as ApiResult};
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::item::{SongItem, TileItem};
use crate::item_actions::{self, SongCatalog};
use crate::services::{ApiClient, AppServices};
use crate::views::detail_frame;
use crate::views::more::MorePage;
use crate::widgets::song_carousel::SongCarousel;
use crate::widgets::tile::{self, TileRow};

/// Time that the entry waits after the last key before it searches.
const SEARCH_DELAY_MS: u32 = 300;

/// Maximum number of results for each type. Apple allows at most 25.
const RESULT_LIMIT: u32 = 25;

/// A section of the search results.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SearchSection {
    Songs,
    Albums,
    Artists,
    Playlists,
}

impl SearchSection {
    /// All sections, in the order of the page.
    pub const ALL: [Self; 4] = [Self::Songs, Self::Albums, Self::Artists, Self::Playlists];

    pub fn title(self) -> String {
        match self {
            Self::Songs => gettext("Songs"),
            Self::Albums => gettext("Albums"),
            Self::Artists => gettext("Artists"),
            Self::Playlists => gettext("Playlists"),
        }
    }

    /// Name of the action that opens all results of the section, without
    /// the `win.` prefix. Its target is the scope name and the term.
    pub fn action_name(self) -> &'static str {
        match self {
            Self::Songs => "open-search-songs",
            Self::Albums => "open-search-albums",
            Self::Artists => "open-search-artists",
            Self::Playlists => "open-search-playlists",
        }
    }

    fn catalog_type(self) -> CatalogSearchType {
        match self {
            Self::Songs => CatalogSearchType::Songs,
            Self::Albums => CatalogSearchType::Albums,
            Self::Artists => CatalogSearchType::Artists,
            Self::Playlists => CatalogSearchType::Playlists,
        }
    }

    fn library_type(self) -> LibrarySearchType {
        match self {
            Self::Songs => LibrarySearchType::Songs,
            Self::Albums => LibrarySearchType::Albums,
            Self::Artists => LibrarySearchType::Artists,
            Self::Playlists => LibrarySearchType::Playlists,
        }
    }
}

/// Where the search looks.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Scope {
    /// The Apple Music catalog.
    #[default]
    Catalog,
    /// The library of the user.
    Library,
}

impl Scope {
    const ALL: [Self; 2] = [Self::Catalog, Self::Library];

    /// Name of the toggle in the scope switch and of the scope in action
    /// targets.
    pub fn name(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Library => "library",
        }
    }

    /// The scope of `name`. An unknown name gives the default scope.
    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|scope| scope.name() == name)
            .unwrap_or_default()
    }

    /// Label of the toggle in the scope switch.
    fn label(self) -> String {
        match self {
            Self::Catalog => gettext("Apple Music"),
            Self::Library => gettext("Library"),
        }
    }

    /// Title of the hint page and placeholder of the entry.
    fn hint(self) -> String {
        match self {
            Self::Catalog => gettext("Search Apple Music"),
            Self::Library => gettext("Search Your Library"),
        }
    }
}

/// The results of one search, ready for the widgets. Each section keeps the
/// offset of its next page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchSections {
    songs: Page<SongItem>,
    albums: Page<TileItem>,
    artists: Page<TileItem>,
    playlists: Page<TileItem>,
}

impl SearchSections {
    fn is_empty(&self) -> bool {
        self.songs.items.is_empty()
            && self.albums.items.is_empty()
            && self.artists.items.is_empty()
            && self.playlists.items.is_empty()
    }

    /// Tells if `section` has more results than this page.
    fn has_more(&self, section: SearchSection) -> bool {
        let next_offset = match section {
            SearchSection::Songs => self.songs.next_offset,
            SearchSection::Albums => self.albums.next_offset,
            SearchSection::Artists => self.artists.next_offset,
            SearchSection::Playlists => self.playlists.next_offset,
        };
        next_offset.is_some()
    }

    /// Takes the page of `section`.
    fn into_more_page(self, section: SearchSection) -> MorePage {
        match section {
            SearchSection::Songs => MorePage::Songs(self.songs),
            SearchSection::Albums => MorePage::Tiles(self.albums),
            SearchSection::Artists => MorePage::Tiles(self.artists),
            SearchSection::Playlists => MorePage::Tiles(self.playlists),
        }
    }
}

impl From<SearchResults> for SearchSections {
    fn from(results: SearchResults) -> Self {
        Self {
            songs: convert(results.songs),
            albums: convert(results.albums),
            artists: convert(results.artists),
            playlists: convert(results.playlists),
        }
    }
}

impl From<LibrarySearchResults> for SearchSections {
    fn from(results: LibrarySearchResults) -> Self {
        Self {
            songs: convert(results.songs),
            albums: convert(results.albums),
            artists: convert(results.artists),
            playlists: convert(results.playlists),
        }
    }
}

/// Converts the items of `page` and keeps the offset of the next page. A
/// missing page is empty.
fn convert<T, U: for<'a> From<&'a T>>(page: Option<ApiPage<T>>) -> Page<U> {
    let page = page.map(Page::from).unwrap_or_default();
    Page {
        items: page.items.iter().map(U::from).collect(),
        next_offset: page.next_offset,
    }
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
    /// The user picked where to search. The last term is searched again.
    ScopeChanged(Scope),
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
    scope: Scope,
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
    /// The Show More buttons, in the order of [`SearchSection::ALL`].
    more_buttons: [gtk::Button; 4],
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
                    set_child = &gtk::Box {
                        set_spacing: 6,

                        append = &search_entry.clone() -> gtk::SearchEntry {},
                        append = &build_scope_switch(&sender) -> adw::ToggleGroup {},
                    },
                },
            },

            #[wrap(Some)]
            set_content = &gtk::Stack {
                #[watch]
                set_visible_child_name: model.phase.page(),

                add_named[Some(Phase::Hint.page())] = &adw::StatusPage {
                    set_icon_name: Some("system-search-symbolic"),
                    #[watch]
                    set_title: &model.scope.hint(),
                    set_description: Some(&gettext("Look for songs, albums, artists, and playlists.")),
                },

                add_named[Some(Phase::Loading.page())] = &detail_frame::loading_spinner() -> adw::Spinner {},

                add_named[Some(Phase::Empty.page())] = &adw::StatusPage {
                    set_icon_name: Some("edit-find-symbolic"),
                    set_title: &gettext("No Results"),
                    set_description: Some(&gettext("Try a different term.")),
                },

                add_named[Some(Phase::Failed.page())] = &failed_status.clone() -> adw::StatusPage {},

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
        let more_buttons = SearchSection::ALL
            .map(|section| tile::more_button(&format!("win.{}", section.action_name())));
        let [more_songs, more_albums, more_artists, more_playlists] = &more_buttons;
        let songs_section = build_songs_section(&songs, more_songs);
        let (albums, artists, playlists) = (
            TileRow::new(&SearchSection::Albums.title()),
            TileRow::new(&SearchSection::Artists.title()),
            TileRow::new(&SearchSection::Playlists.title()),
        );
        albums.append_header_button(more_albums);
        artists.append_header_button(more_artists);
        playlists.append_header_button(more_playlists);
        let results_page = build_results_page(&songs_section, [&albums, &artists, &playlists]);

        let model = Self {
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            entry: search_entry.clone(),
            phase: Phase::Hint,
            scope: Scope::default(),
            term: String::new(),
            generation: 0,
            pending: None,
            songs,
            song_items: Vec::new(),
            songs_section,
            albums,
            artists,
            playlists,
            more_buttons,
        };
        let input = sender.input_sender().clone();
        let failed_status = detail_frame::failed_page(&gettext("Search Failed"), move || {
            input.emit(SearchInput::Retry);
        });
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SearchInput::TermChanged(term) => self.search(&term, &sender),
            SearchInput::ScopeChanged(scope) => {
                self.scope = scope;
                self.entry.set_placeholder_text(Some(&scope.hint()));
                let term = self.term.clone();
                self.search(&term, &sender);
            }
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
        let storefront = self.core.storefront().unwrap_or_default();
        self.phase = Phase::Loading;
        let (api, term, generation) = (Arc::clone(&self.api), self.term.clone(), self.generation);
        let scope = self.scope;
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let sections = fetch(&api, scope, &storefront, &term).await;
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

    /// Puts `sections` in the widgets. An empty section hides itself. A
    /// section with more results shows its Show More button.
    fn show(&mut self, sections: SearchSections) {
        let target = (self.scope.name(), self.term.as_str()).to_variant();
        for (section, button) in SearchSection::ALL.into_iter().zip(&self.more_buttons) {
            button.set_action_target_value(Some(&target));
            button.set_visible(sections.has_more(section));
        }
        self.catalog.remember(&sections.songs.items);
        self.songs.set_songs(&sections.songs.items);
        self.songs_section
            .set_visible(!sections.songs.items.is_empty());
        self.albums.set_items(&sections.albums.items);
        self.artists.set_items(&sections.artists.items);
        self.playlists.set_items(&sections.playlists.items);
        self.song_items = sections.songs.items;
    }
}

/// Searches `scope` for `term`. Returns `None` if the request fails.
async fn fetch(
    api: &ApiClient,
    scope: Scope,
    storefront: &str,
    term: &str,
) -> Option<SearchSections> {
    request(api, scope, storefront, term, &SearchSection::ALL, 0)
        .await
        .inspect_err(|error| warn!(%error, ?scope, "search failed"))
        .ok()
}

/// Loads the results of `section` from `offset`, for the page with all
/// results of the section. The catalog search needs `storefront`.
///
/// # Errors
///
/// Returns an error if the request fails.
pub async fn load_section(
    api: &ApiClient,
    scope: Scope,
    storefront: &str,
    term: &str,
    section: SearchSection,
    offset: u32,
) -> ApiResult<MorePage> {
    let sections = request(api, scope, storefront, term, &[section], offset).await?;
    Ok(sections.into_more_page(section))
}

/// Searches `scope` for `term` in `sections`, from the result at `offset`.
/// The catalog search needs `storefront`. The library search ignores it.
async fn request(
    api: &ApiClient,
    scope: Scope,
    storefront: &str,
    term: &str,
    sections: &[SearchSection],
    offset: u32,
) -> ApiResult<SearchSections> {
    let results = match scope {
        Scope::Catalog => {
            let types: Vec<CatalogSearchType> = sections
                .iter()
                .copied()
                .map(SearchSection::catalog_type)
                .collect();
            let results =
                endpoints::search(api, storefront, term, &types, RESULT_LIMIT, offset).await?;
            SearchSections::from(results)
        }
        Scope::Library => {
            let types: Vec<LibrarySearchType> = sections
                .iter()
                .copied()
                .map(SearchSection::library_type)
                .collect();
            let results =
                endpoints::library_search(api, term, &types, RESULT_LIMIT, offset).await?;
            SearchSections::from(results)
        }
    };
    Ok(results)
}

/// Builds the entry. It sends the term after a short pause, and at once on Enter.
fn build_entry(sender: &ComponentSender<SearchView>) -> gtk::SearchEntry {
    let entry = gtk::SearchEntry::builder()
        .hexpand(true)
        .search_delay(SEARCH_DELAY_MS)
        .placeholder_text(Scope::default().hint())
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

/// Builds the switch between the catalog and the library.
fn build_scope_switch(sender: &ComponentSender<SearchView>) -> adw::ToggleGroup {
    let switch = adw::ToggleGroup::new();
    for scope in Scope::ALL {
        let toggle = adw::Toggle::builder()
            .name(scope.name())
            .label(scope.label())
            .build();
        switch.add(toggle);
    }
    switch.set_active_name(Some(Scope::default().name()));
    let input = sender.input_sender().clone();
    switch.connect_active_name_notify(move |switch| {
        let name = switch.active_name().unwrap_or_default();
        input.emit(SearchInput::ScopeChanged(Scope::from_name(&name)));
    });
    switch
}

/// Builds the songs heading with the Show More button, and the list under
/// it. It hides itself when empty.
fn build_songs_section(carousel: &SongCarousel, more: &gtk::Button) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .visible(false)
        .build();
    let title_row = tile::heading_row(&SearchSection::Songs.title());
    title_row.append(more);
    section.append(&title_row);
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
        let sections = SearchSections::from(results(json!({})));
        assert!(sections.is_empty());
    }

    #[test]
    fn sections_keep_the_order_of_the_songs() {
        let value = json!({ "songs": { "data": [song_json("1"), song_json("2")] } });
        let sections = SearchSections::from(results(value));
        let ids: Vec<&str> = sections.songs.items.iter().map(SongItem::id).collect();
        assert_eq!(ids, ["1", "2"]);
        assert!(!sections.is_empty());
    }

    #[test]
    fn library_sections_convert_all_types() {
        let value = json!({
            "library-songs": { "data": [{ "id": "i.1", "type": "library-songs", "attributes": { "name": "S" } }] },
            "library-albums": { "data": [{ "id": "l.a", "type": "library-albums", "attributes": { "name": "A" } }] },
            "library-artists": { "data": [{ "id": "r.b", "type": "library-artists", "attributes": { "name": "B" } }] },
            "library-playlists": { "data": [{ "id": "p.c", "type": "library-playlists", "attributes": { "name": "C" } }] }
        });
        let results: LibrarySearchResults =
            serde_json::from_value(value).expect("the test results are valid");
        let sections = SearchSections::from(results);
        assert_eq!(
            (
                sections.songs.items.len(),
                sections.albums.items.len(),
                sections.artists.items.len(),
                sections.playlists.items.len()
            ),
            (1, 1, 1, 1)
        );
    }

    #[test]
    fn has_more_follows_the_next_link_of_each_section() {
        let value = json!({
            "songs": { "data": [song_json("1")], "next": "/v1/catalog/it/search?offset=25&term=x&types=songs" },
            "albums": { "data": [] }
        });
        let sections = SearchSections::from(results(value));
        let more: Vec<bool> = SearchSection::ALL
            .into_iter()
            .map(|section| sections.has_more(section))
            .collect();
        assert_eq!(more, [true, false, false, false]);
    }

    #[test]
    fn into_more_page_keeps_the_next_offset() {
        let value = json!({
            "songs": { "data": [song_json("1")], "next": "/v1/catalog/it/search?offset=25&term=x&types=songs" }
        });
        let MorePage::Songs(page) =
            SearchSections::from(results(value)).into_more_page(SearchSection::Songs)
        else {
            panic!("the songs section gives a page of songs");
        };
        assert_eq!((page.items.len(), page.next_offset), (1, Some(25)));
    }

    #[test]
    fn scope_names_round_trip() {
        for scope in Scope::ALL {
            assert_eq!(Scope::from_name(scope.name()), scope);
        }
    }

    #[test]
    fn sections_convert_albums_artists_and_playlists() {
        let value = json!({
            "albums": { "data": [{ "id": "a", "type": "albums", "attributes": { "name": "A" } }] },
            "artists": { "data": [{ "id": "b", "type": "artists", "attributes": { "name": "B" } }] },
            "playlists": { "data": [{ "id": "c", "type": "playlists", "attributes": { "name": "C" } }] }
        });
        let sections = SearchSections::from(results(value));
        assert_eq!(
            (
                sections.albums.items.len(),
                sections.artists.items.len(),
                sections.playlists.items.len()
            ),
            (1, 1, 1)
        );
    }
}
