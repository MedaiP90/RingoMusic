// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Artist page: the header, the top songs, and the albums.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::detail::{self, ArtistData};
use crate::item::SongItem;
use crate::item_actions::{self, OPEN_ARTIST_ALBUMS, OPEN_ARTIST_SONGS, SongCatalog};
use crate::services::{ApiClient, AppServices};
use crate::views::detail_frame::DetailFrame;
use crate::views::phase::Phase;
use crate::widgets::detail_header::{ArtworkShape, DetailHeader, PlayActions};
use crate::widgets::song_list::SongList;
use crate::widgets::song_row::Lead;
use crate::widgets::tile::{TileGrid, heading};

/// What an artist page needs to start.
pub struct ArtistInit {
    pub services: AppServices,
    pub catalog: SongCatalog,
    /// ID of a catalog artist or a library artist.
    pub id: String,
}

/// Input of an artist page.
#[derive(Debug)]
pub enum ArtistInput {
    /// Loads the artist again.
    Retry,
    /// A load ended. `None` is a failure.
    Loaded(Option<ArtistData>),
    /// The user pressed the top song at the position.
    PlaySong(usize),
    PlayTopSongs,
}

/// An artist page.
pub struct ArtistView {
    id: String,
    api: Arc<ApiClient>,
    core: CoreHandle,
    catalog: SongCatalog,
    title: String,
    top_songs: Vec<SongItem>,
    header: DetailHeader,
    frame: DetailFrame,
    actions: PlayActions,
    songs_section: gtk::Box,
    song_list: SongList,
    albums: TileGrid,
    /// Open the pages with all top songs and all albums.
    more_songs: gtk::Button,
    more_albums: gtk::Button,
    pending: Option<JoinHandle<()>>,
}

#[relm4::component(pub)]
impl SimpleComponent for ArtistView {
    type Init = ArtistInit;
    type Input = ArtistInput;
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
        let ArtistInit {
            services,
            catalog,
            id,
        } = init;
        let header = DetailHeader::new(ArtworkShape::Round);
        header.set_shuffle_visible(false);
        let input = sender.input_sender().clone();
        let song_list = SongList::new(move |position| input.emit(ArtistInput::PlaySong(position)));
        let more_songs = more_button(OPEN_ARTIST_SONGS);
        let songs_section = build_songs_section(&song_list, &more_songs);
        let albums = TileGrid::new(&gettext("Albums"));
        let more_albums = more_button(OPEN_ARTIST_ALBUMS);
        albums.append_footer(&more_albums);
        let content = build_content(&header, &songs_section, &albums);
        let input = sender.input_sender().clone();
        let frame = DetailFrame::new(
            &content,
            &gettext("Nothing to show for this artist"),
            move || {
                input.emit(ArtistInput::Retry);
            },
        );
        let frame_widget = frame.widget().clone();
        let play = sender.input_sender().clone();
        let actions =
            PlayActions::install(&root, move || play.emit(ArtistInput::PlayTopSongs), || {});

        let mut model = Self {
            id,
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            title: gettext("Artist"),
            top_songs: Vec::new(),
            header,
            frame,
            actions,
            songs_section,
            song_list,
            albums,
            more_songs,
            more_albums,
            pending: None,
        };
        model.reload(&sender);
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ArtistInput::Retry => self.reload(&sender),
            ArtistInput::Loaded(Some(data)) => self.show(&data),
            ArtistInput::Loaded(None) => self.set_phase(Phase::Failed, false),
            ArtistInput::PlaySong(position) => {
                item_actions::play_songs(&self.core, &self.top_songs, position);
            }
            ArtistInput::PlayTopSongs => item_actions::play_songs(&self.core, &self.top_songs, 0),
        }
    }
}

impl ArtistView {
    fn reload(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.set_phase(Phase::Loading, false);
        let storefront = detail::storefront(&self.core.subscribe().borrow()).unwrap_or_default();
        let (api, id) = (Arc::clone(&self.api), self.id.clone());
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let data = match detail::load_artist(&api, &storefront, &id).await {
                Ok(data) => Some(data),
                Err(error) => {
                    warn!(%error, id, "cannot load the artist");
                    None
                }
            };
            input.emit(ArtistInput::Loaded(data));
        }));
    }

    /// Shows the page of `phase`. The Play action works only when the page
    /// has top songs.
    fn set_phase(&mut self, phase: Phase, can_play: bool) {
        self.frame.show(phase);
        self.actions.set_enabled(can_play);
    }

    /// Puts the artist in the widgets.
    fn show(&mut self, data: &ArtistData) {
        self.title.clone_from(&data.name);
        self.top_songs.clone_from(&data.top_songs);
        self.catalog.remember(&self.top_songs);
        self.header.set_artwork(data.artwork_template.as_deref());
        self.header.set_title(&data.name);
        self.song_list
            .set_songs(self.top_songs.iter().map(|song| (song, Lead::Artwork)));
        self.songs_section.set_visible(!self.top_songs.is_empty());
        self.albums.set_items(&data.albums);
        let target = item_actions::artist_list_target(&self.id, &data.name);
        for (button, has_more) in [
            (&self.more_songs, data.top_songs_next.is_some()),
            (&self.more_albums, data.albums_next.is_some()),
        ] {
            button.set_action_target_value(Some(&target));
            button.set_visible(has_more);
        }
        let is_empty = self.top_songs.is_empty() && data.albums.is_empty();
        let phase = if is_empty {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.set_phase(phase, !self.top_songs.is_empty());
    }
}

/// Builds a hidden Show More button that opens a page with all items. `show`
/// gives it the target.
fn more_button(action: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(gettext("Show More"))
        .halign(gtk::Align::Center)
        .margin_top(6)
        .css_classes(["pill"])
        .action_name(action)
        .visible(false)
        .build()
}

/// Builds the heading, the list of the top songs, and the Show More button.
fn build_songs_section(list: &SongList, more: &gtk::Button) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .visible(false)
        .build();
    section.append(&heading(&gettext("Top Songs")));
    section.append(list.widget());
    section.append(more);
    section
}

/// Builds the scrolling content: the header, the top songs, and the albums.
fn build_content(
    header: &DetailHeader,
    songs: &gtk::Box,
    albums: &TileGrid,
) -> gtk::ScrolledWindow {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_bottom(18)
        .build();
    page.append(header.widget());
    page.append(songs);
    page.append(albums.widget());
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&page)
        .build()
}
