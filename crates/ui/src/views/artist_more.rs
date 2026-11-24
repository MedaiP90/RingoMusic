// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Page with all top songs or all tiles of one list of an artist, for
//! example all singles. The items load page by
//! page when the user scrolls near the end.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use ringo_am_api::Page;
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::detail::{self, ArtistList};
use crate::item::{SongItem, TileItem};
use crate::item_actions::{self, SongCatalog};
use crate::paged::{self, PageCursor};
use crate::services::{ApiClient, AppServices};
use crate::views::detail_frame::DetailFrame;
use crate::views::phase::Phase;
use crate::widgets::song_list::SongList;
use crate::widgets::song_row::Lead;
use crate::widgets::tile::TileGrid;

/// Size of the spinner below the list, in pixels.
const MORE_SPINNER_SIZE: i32 = 24;

/// What the page lists.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MoreKind {
    TopSongs,
    Tiles(ArtistList),
}

impl MoreKind {
    fn title(self) -> String {
        match self {
            Self::TopSongs => gettext("Top Songs"),
            Self::Tiles(list) => list.title(),
        }
    }

    /// Makes the empty list of this kind. A press on a song sends its position.
    fn build_list(
        self,
        catalog: SongCatalog,
        input: relm4::Sender<ArtistMoreInput>,
    ) -> Box<dyn MoreList> {
        match self {
            Self::TopSongs => Box::new(SongsList::new(catalog, input)),
            Self::Tiles(list) => Box::new(TilesList::new(&list.title())),
        }
    }

    /// Loads the page of this kind at `offset`.
    async fn load(
        self,
        api: &ApiClient,
        storefront: &str,
        id: &str,
        offset: u32,
    ) -> ringo_am_api::Result<MorePage> {
        match self {
            Self::TopSongs => detail::load_artist_top_songs(api, storefront, id, offset)
                .await
                .map(MorePage::Songs),
            Self::Tiles(list) => detail::load_artist_tiles(api, storefront, id, list, offset)
                .await
                .map(MorePage::Tiles),
        }
    }
}

/// What the page needs to start.
pub struct ArtistMoreInit {
    pub services: AppServices,
    pub catalog: SongCatalog,
    /// ID of a catalog artist.
    pub id: String,
    /// Shown in the title.
    pub artist_name: String,
    pub kind: MoreKind,
}

/// A loaded page of items.
#[derive(Debug)]
pub enum MorePage {
    Songs(Page<SongItem>),
    Tiles(Page<TileItem>),
}

impl MorePage {
    fn next_offset(&self) -> Option<u32> {
        match self {
            Self::Songs(page) => page.next_offset,
            Self::Tiles(page) => page.next_offset,
        }
    }
}

/// Input of the page.
#[derive(Debug)]
pub enum ArtistMoreInput {
    /// Loads the list again.
    Retry,
    /// The user scrolled near the end of the list.
    NearEnd,
    /// A page loaded. `None` is a failure. The number tells which list it was for.
    PageLoaded(u64, Option<MorePage>),
    /// The user pressed the song at the position.
    PlaySong(usize),
}

/// The widgets that show the items of one kind.
trait MoreList {
    /// The widget to put in the page.
    fn widget(&self) -> gtk::Widget;
    fn clear(&mut self);
    /// Adds the items of `page`. Returns how many it added. A page of
    /// another kind adds none.
    fn append(&mut self, page: MorePage) -> usize;
    /// Plays the item at `position`, if the items are songs.
    fn play(&self, core: &CoreHandle, position: usize);
}

struct SongsList {
    list: SongList,
    songs: Vec<SongItem>,
    catalog: SongCatalog,
}

impl SongsList {
    fn new(catalog: SongCatalog, input: relm4::Sender<ArtistMoreInput>) -> Self {
        let list = SongList::new(move |position| input.emit(ArtistMoreInput::PlaySong(position)));
        Self {
            list,
            songs: Vec::new(),
            catalog,
        }
    }
}

impl MoreList for SongsList {
    fn widget(&self) -> gtk::Widget {
        self.list.widget().clone().upcast()
    }

    fn clear(&mut self) {
        self.songs.clear();
        self.list.set_songs([]);
    }

    fn append(&mut self, page: MorePage) -> usize {
        let MorePage::Songs(page) = page else {
            return 0;
        };
        self.catalog.remember(&page.items);
        self.list
            .append_songs(page.items.iter().map(|song| (song, Lead::Artwork)));
        let added = page.items.len();
        self.songs.extend(page.items);
        added
    }

    fn play(&self, core: &CoreHandle, position: usize) {
        item_actions::play_songs(core, &self.songs, position);
    }
}

struct TilesList {
    grid: TileGrid,
}

impl TilesList {
    fn new(title: &str) -> Self {
        Self {
            grid: TileGrid::new(title),
        }
    }
}

impl MoreList for TilesList {
    fn widget(&self) -> gtk::Widget {
        self.grid.widget().clone().upcast()
    }

    fn clear(&mut self) {
        self.grid.set_items(&[]);
    }

    fn append(&mut self, page: MorePage) -> usize {
        let MorePage::Tiles(page) = page else {
            return 0;
        };
        self.grid.append_items(&page.items);
        page.items.len()
    }

    fn play(&self, _core: &CoreHandle, _position: usize) {}
}

/// The page of all top songs or all tiles of one list of an artist.
pub struct ArtistMoreView {
    id: String,
    kind: MoreKind,
    api: Arc<ApiClient>,
    core: CoreHandle,
    phase: Phase,
    cursor: PageCursor,
    item_count: usize,
    list: Box<dyn MoreList>,
    frame: DetailFrame,
    /// Shows below the last item while a later page loads.
    more_spinner: adw::Spinner,
    pending: Option<JoinHandle<()>>,
}

#[relm4::component(pub)]
impl SimpleComponent for ArtistMoreView {
    type Init = ArtistMoreInit;
    type Input = ArtistMoreInput;
    type Output = ();

    view! {
        adw::NavigationPage {
            set_title: &title,

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &frame_widget.clone() -> gtk::Stack {},
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let ArtistMoreInit {
            services,
            catalog,
            id,
            artist_name,
            kind,
        } = init;
        let title = format!("{artist_name} - {}", kind.title());
        let list = kind.build_list(catalog.clone(), sender.input_sender().clone());
        let list_widget = list.widget();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list_widget)
            .build();
        let near_end = sender.input_sender().clone();
        paged::connect_near_end(&scrolled, move || near_end.emit(ArtistMoreInput::NearEnd));
        let more_spinner = build_more_spinner();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&scrolled);
        content.append(&more_spinner);
        let retry = sender.input_sender().clone();
        let frame = DetailFrame::new(&content, &gettext("Nothing to show"), move || {
            retry.emit(ArtistMoreInput::Retry);
        });
        let frame_widget = frame.widget().clone();

        let mut model = Self {
            id,
            kind,
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            phase: Phase::Idle,
            cursor: PageCursor::default(),
            item_count: 0,
            list,
            frame,
            more_spinner,
            pending: None,
        };
        model.reload(&sender);
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ArtistMoreInput::Retry => self.reload(&sender),
            ArtistMoreInput::NearEnd => {
                if self.phase == Phase::Content {
                    self.load_next(&sender);
                }
            }
            ArtistMoreInput::PageLoaded(generation, page) => self.finish_page(generation, page),
            ArtistMoreInput::PlaySong(position) => {
                self.list.play(&self.core, position);
            }
        }
    }
}

impl ArtistMoreView {
    /// Clears the page and loads the list from the first page.
    fn reload(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.cursor.reset();
        self.item_count = 0;
        self.list.clear();
        self.more_spinner.set_visible(false);
        self.set_phase(Phase::Loading);
        self.load_next(sender);
    }

    fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.frame.show(phase);
    }

    /// Requests the next page, if no load runs and a page is left.
    fn load_next(&mut self, sender: &ComponentSender<Self>) {
        let Some((generation, offset)) = self.cursor.begin() else {
            return;
        };
        self.more_spinner.set_visible(self.phase == Phase::Content);
        let (api, id, kind) = (Arc::clone(&self.api), self.id.clone(), self.kind);
        let storefront = detail::storefront(&self.core.subscribe().borrow()).unwrap_or_default();
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let page = match kind.load(&api, &storefront, &id, offset).await {
                Ok(page) => Some(page),
                Err(error) => {
                    warn!(%error, id, offset, "cannot load a page of the artist");
                    None
                }
            };
            input.emit(ArtistMoreInput::PageLoaded(generation, page));
        }));
    }

    /// Adds a loaded page. A failure keeps the items. The next scroll tries
    /// the page again. A failure of the first page shows the error.
    fn finish_page(&mut self, generation: u64, page: Option<MorePage>) {
        self.more_spinner.set_visible(false);
        let Some(page) = page else {
            if self.cursor.fail(generation) && self.item_count == 0 {
                self.set_phase(Phase::Failed);
            }
            return;
        };
        if !self.cursor.finish(generation, page.next_offset()) {
            return;
        }
        self.item_count += self.list.append(page);
        let phase = if self.item_count == 0 {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.set_phase(phase);
    }
}

/// Builds the spinner that tells the user that more items are loading.
fn build_more_spinner() -> adw::Spinner {
    adw::Spinner::builder()
        .halign(gtk::Align::Center)
        .width_request(MORE_SPINNER_SIZE)
        .height_request(MORE_SPINNER_SIZE)
        .margin_top(12)
        .margin_bottom(12)
        .visible(false)
        .build()
}
