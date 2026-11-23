// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Playlist page: the header, and the songs. The songs load page by page
//! when the user scrolls near the end.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use relm4::typed_view::list::TypedListView;
use ringo_am_api::Page;
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::detail::{self, PlaylistData};
use crate::item::SongItem;
use crate::item_actions::{self, SongCatalog};
use crate::paged::{self, PageCursor};
use crate::services::{ApiClient, AppServices};
use crate::views::detail_frame::DetailFrame;
use crate::views::phase::Phase;
use crate::widgets::detail_header::{ArtworkShape, DetailHeader, PlayActions};
use crate::widgets::song_row::{self, SongRow};

/// Size of the spinner below the list, in pixels.
const MORE_SPINNER_SIZE: i32 = 24;

/// What a playlist page needs to start.
pub struct PlaylistInit {
    pub services: AppServices,
    pub catalog: SongCatalog,
    /// ID of a catalog playlist or a library playlist.
    pub id: String,
}

/// How the Play button orders the songs.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PlayOrder {
    InOrder,
    Shuffled,
}

/// Input of a playlist page.
#[derive(Debug)]
pub enum PlaylistInput {
    /// Loads the playlist again.
    Retry,
    /// The first load ended. The number tells which load it was for.
    Loaded(u64, Option<PlaylistData>),
    /// The user scrolled near the end of the list.
    NearEnd,
    /// A later page loaded.
    PageLoaded(u64, Option<Page<SongItem>>),
    /// The user pressed the song at the position.
    PlaySong(u32),
    /// Plays all songs, also the pages that are not loaded yet.
    PlayAll(PlayOrder),
}

/// A playlist page.
pub struct PlaylistView {
    id: String,
    api: Arc<ApiClient>,
    core: CoreHandle,
    catalog: SongCatalog,
    title: String,
    phase: Phase,
    cursor: PageCursor,
    songs: Vec<SongItem>,
    header: DetailHeader,
    frame: DetailFrame,
    actions: PlayActions,
    list: TypedListView<SongRow, gtk::SingleSelection>,
    /// Shows below the last song while a later page loads.
    more_spinner: adw::Spinner,
    pending: Option<JoinHandle<()>>,
}

#[relm4::component(pub)]
impl SimpleComponent for PlaylistView {
    type Init = PlaylistInit;
    type Input = PlaylistInput;
    type Output = ();

    view! {
        adw::NavigationPage {
            #[watch]
            set_title: &model.title,

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
        let PlaylistInit {
            services,
            catalog,
            id,
        } = init;
        let header = DetailHeader::new(ArtworkShape::Square);
        let list = build_list(&sender);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list.view)
            .build();
        let input = sender.input_sender().clone();
        paged::connect_near_end(&scrolled, move || input.emit(PlaylistInput::NearEnd));
        let more_spinner = build_more_spinner();
        let content = build_content(&header, &scrolled, &more_spinner);
        let input = sender.input_sender().clone();
        let frame = DetailFrame::new(
            &content,
            &gettext("This playlist has no songs"),
            move || {
                input.emit(PlaylistInput::Retry);
            },
        );
        let frame_widget = frame.widget().clone();
        let (play, shuffle) = (sender.input_sender().clone(), sender.input_sender().clone());
        let actions = PlayActions::install(
            &root,
            move || play.emit(PlaylistInput::PlayAll(PlayOrder::InOrder)),
            move || shuffle.emit(PlaylistInput::PlayAll(PlayOrder::Shuffled)),
        );

        let mut model = Self {
            id,
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            title: gettext("Playlist"),
            phase: Phase::Idle,
            cursor: PageCursor::default(),
            songs: Vec::new(),
            header,
            frame,
            actions,
            list,
            more_spinner,
            pending: None,
        };
        model.reload(&sender);
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            PlaylistInput::Retry => self.reload(&sender),
            PlaylistInput::Loaded(generation, data) => self.finish_first_load(generation, data),
            PlaylistInput::NearEnd => {
                if self.phase == Phase::Content {
                    self.load_next(&sender);
                }
            }
            PlaylistInput::PageLoaded(generation, page) => self.finish_page(generation, page),
            PlaylistInput::PlaySong(position) => {
                let start = usize::try_from(position).unwrap_or(usize::MAX);
                item_actions::play_songs(&self.core, &self.songs, start);
            }
            PlaylistInput::PlayAll(order) => self.play_all(order),
        }
    }
}

impl PlaylistView {
    /// Clears the page and loads the playlist from the first page.
    fn reload(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.cursor.reset();
        self.songs.clear();
        self.list.clear();
        self.more_spinner.set_visible(false);
        self.set_phase(Phase::Loading);
        let Some((generation, _)) = self.cursor.begin() else {
            return;
        };
        let (api, id) = (Arc::clone(&self.api), self.id.clone());
        let storefront = self.storefront();
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let data = match detail::load_playlist(&api, &storefront, &id).await {
                Ok(data) => Some(data),
                Err(error) => {
                    warn!(%error, id, "cannot load the playlist");
                    None
                }
            };
            input.emit(PlaylistInput::Loaded(generation, data));
        }));
    }

    fn storefront(&self) -> String {
        detail::storefront(&self.core.subscribe().borrow()).unwrap_or_default()
    }

    fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.frame.show(phase);
        self.actions.set_enabled(phase == Phase::Content);
    }

    /// Shows the header and the first songs, or the failure.
    fn finish_first_load(&mut self, generation: u64, data: Option<PlaylistData>) {
        let Some(data) = data else {
            if self.cursor.fail(generation) {
                self.set_phase(Phase::Failed);
            }
            return;
        };
        if !self.cursor.finish(generation, data.first_page.next_offset) {
            return;
        }
        self.title.clone_from(&data.title);
        self.header.set_artwork(data.artwork_template.as_deref());
        self.header.set_title(&data.title);
        self.header.set_byline(&data.curator, None);
        self.header.set_details("");
        self.header.set_description(data.description.as_deref());
        self.append(data.first_page.items);
        let phase = if self.songs.is_empty() {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.set_phase(phase);
    }

    /// Requests the next page of songs, if no load runs and a page is left.
    fn load_next(&mut self, sender: &ComponentSender<Self>) {
        let Some((generation, offset)) = self.cursor.begin() else {
            return;
        };
        self.more_spinner.set_visible(true);
        let (api, id) = (Arc::clone(&self.api), self.id.clone());
        let storefront = self.storefront();
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let page = match detail::load_playlist_songs(&api, &storefront, &id, offset).await {
                Ok(page) => Some(page),
                Err(error) => {
                    warn!(%error, id, offset, "cannot load a page of playlist songs");
                    None
                }
            };
            input.emit(PlaylistInput::PageLoaded(generation, page));
        }));
    }

    /// Adds a loaded page. A failure keeps the songs. The next scroll tries
    /// the page again.
    fn finish_page(&mut self, generation: u64, page: Option<Page<SongItem>>) {
        self.more_spinner.set_visible(false);
        let Some(page) = page else {
            self.cursor.fail(generation);
            return;
        };
        if self.cursor.finish(generation, page.next_offset) {
            self.append(page.items);
        }
    }

    fn append(&mut self, songs: Vec<SongItem>) {
        self.catalog.remember(&songs);
        for song in &songs {
            self.list.append(SongRow::new(song.clone()));
        }
        self.songs.extend(songs);
    }

    /// Plays all songs of the playlist. The pages that are not loaded yet load
    /// first. A failed page ends the list: the songs so far play.
    fn play_all(&self, order: PlayOrder) {
        let mut songs = self.songs.clone();
        let next_offset = self.cursor.next_offset();
        let (api, id, core) = (Arc::clone(&self.api), self.id.clone(), self.core.clone());
        let storefront = self.storefront();
        relm4::spawn(async move {
            let mut offset = next_offset;
            while let Some(current) = offset {
                match detail::load_playlist_songs(&api, &storefront, &id, current).await {
                    Ok(page) => {
                        songs.extend(page.items);
                        offset = page.next_offset;
                    }
                    Err(error) => {
                        warn!(%error, id, offset = current, "cannot load all playlist songs");
                        break;
                    }
                }
            }
            match order {
                PlayOrder::InOrder => item_actions::play_songs(&core, &songs, 0),
                PlayOrder::Shuffled => item_actions::shuffle_songs(&core, &songs),
            }
        });
    }
}

/// Makes the empty song list. A press on a song sends its position.
fn build_list(
    sender: &ComponentSender<PlaylistView>,
) -> TypedListView<SongRow, gtk::SingleSelection> {
    let list = TypedListView::<SongRow, gtk::SingleSelection>::new();
    list.view.set_show_separators(true);
    list.view.set_single_click_activate(true);
    let input = sender.input_sender().clone();
    list.view
        .connect_activate(move |_, position| input.emit(PlaylistInput::PlaySong(position)));
    song_row::connect_menu_key(&list.view);
    list
}

/// Builds the spinner that tells the user that more songs are loading.
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

/// Builds the content: the header above the scrolling song list.
fn build_content(
    header: &DetailHeader,
    scrolled: &gtk::ScrolledWindow,
    more_spinner: &adw::Spinner,
) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.append(header.widget());
    page.append(scrolled);
    page.append(more_spinner);
    page
}
