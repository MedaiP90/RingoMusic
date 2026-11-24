// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Artist page: the header with the genre, the origin, and the start year,
//! the top songs, and one row for each list of the artist, for example the
//! albums, the singles, and the similar artists. A list without items is
//! hidden. The page takes a tint
//! from the artist photo. The Info button opens all the artist information.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::glib::Variant;
use relm4::prelude::*;
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::detail::{self, ArtistData, ArtistList, ArtistStart};
use crate::item::SongItem;
use crate::item_actions::{self, OPEN_ARTIST_SONGS, SongCatalog};
use crate::services::{ApiClient, AppServices};
use crate::tint;
use crate::views::artist_info;
use crate::views::detail_frame::DetailFrame;
use crate::views::phase::Phase;
use crate::widgets::detail_header::{ArtworkShape, DetailHeader, PlayActions};
use crate::widgets::song_carousel::SongCarousel;
use crate::widgets::tile::{TileRow, heading_row};

/// Size of the artwork of a similar artist, in pixels. It is smaller than the
/// album artwork, like in Apple Music.
const SIMILAR_ARTIST_ARTWORK_SIZE: i32 = 100;

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
    Loaded(Option<Box<ArtistData>>),
    /// The user pressed the top song at the position.
    PlaySong(usize),
    PlayTopSongs,
    /// Opens the dialog with all the artist information.
    ShowInfo,
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
    song_list: SongCarousel,
    /// Opens the page with all top songs.
    more_songs: gtk::Button,
    /// One row for each list, in the order of [`ArtistList::ALL`].
    rows: Vec<ListRow>,
    /// Opens the information dialog. Hidden when the artist has no information.
    info_button: gtk::Button,
    /// The loaded artist, for the information dialog.
    data: Option<Box<ArtistData>>,
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
                add_css_class: "album-tint",
                add_top_bar = &adw::HeaderBar {
                    add_css_class: "flat",
                },

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
        tint::follow_viewed(header.artwork_image(), &root, &id);
        let info_button = build_info_button();
        let input = sender.input_sender().clone();
        info_button.connect_clicked(move |_| input.emit(ArtistInput::ShowInfo));
        header.append_button(&info_button);
        let input = sender.input_sender().clone();
        let song_list =
            SongCarousel::new(move |position| input.emit(ArtistInput::PlaySong(position)));
        let more_songs = more_button(OPEN_ARTIST_SONGS);
        let songs_section = build_songs_section(&song_list, &more_songs);
        let rows: Vec<ListRow> = ArtistList::ALL.into_iter().map(ListRow::new).collect();
        let content = build_content(&header, &songs_section, &rows);
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
            more_songs,
            rows,
            info_button,
            data: None,
            pending: None,
        };
        model.reload(&sender);
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ArtistInput::Retry => self.reload(&sender),
            ArtistInput::Loaded(Some(data)) => {
                self.show(&data);
                self.data = Some(data);
            }
            ArtistInput::Loaded(None) => self.set_phase(Phase::Failed, false),
            ArtistInput::PlaySong(position) => {
                item_actions::play_songs(&self.core, &self.top_songs, position);
            }
            ArtistInput::PlayTopSongs => item_actions::play_songs(&self.core, &self.top_songs, 0),
            ArtistInput::ShowInfo => {
                if let Some(data) = &self.data {
                    artist_info::present(self.header.widget(), data, self.header.artwork_image());
                }
            }
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
                Ok(data) => Some(Box::new(data)),
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
        let details = details_line(data);
        self.header.set_details(&details);
        self.info_button
            .set_visible(!details.is_empty() || data.biography.is_some());
        self.song_list.set_songs(&self.top_songs);
        self.songs_section.set_visible(!self.top_songs.is_empty());
        let target = item_actions::artist_list_target(&self.id, &data.name);
        self.more_songs.set_action_target_value(Some(&target));
        self.more_songs.set_visible(data.top_songs_next.is_some());
        for row in &self.rows {
            row.show(data, &target);
        }
        let is_empty =
            self.top_songs.is_empty() && data.lists.iter().all(|tiles| tiles.items.is_empty());
        let phase = if is_empty {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.set_phase(phase, !self.top_songs.is_empty());
    }
}

/// The row of one artist list, with its Show More button.
struct ListRow {
    list: ArtistList,
    tiles: TileRow,
    more: gtk::Button,
}

impl ListRow {
    fn new(list: ArtistList) -> Self {
        let tiles = if list == ArtistList::SimilarArtists {
            let tiles = TileRow::new_with_artwork_size(&list.title(), SIMILAR_ARTIST_ARTWORK_SIZE);
            // More space between the row above and the similar artists.
            tiles.widget().set_margin_top(12);
            tiles
        } else {
            TileRow::new(&list.title())
        };
        let more = more_button(&format!(
            "win.{}",
            item_actions::open_artist_list_name(list)
        ));
        tiles.append_header_button(&more);
        Self { list, tiles, more }
    }

    /// Shows the tiles of the list in `data`. The row hides itself when the
    /// list is empty. `target` is the parameter of the Show More action.
    fn show(&self, data: &ArtistData, target: &Variant) {
        let tiles = data.tiles(self.list);
        self.tiles.set_items(
            tiles
                .map(|tiles| tiles.items.as_slice())
                .unwrap_or_default(),
        );
        self.more.set_action_target_value(Some(target));
        self.more
            .set_visible(tiles.is_some_and(|tiles| tiles.next_offset.is_some()));
    }
}

/// Builds a small hidden Show More button for the right end of a heading. It
/// opens a page with all items. `show` gives it the target.
fn more_button(action: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(gettext("Show More"))
        .valign(gtk::Align::Center)
        .margin_end(12)
        .css_classes(["flat", "section-more"])
        .action_name(action)
        .visible(false)
        .build()
}

/// Builds the heading with the Show More button, and the carousel of the top
/// songs.
fn build_songs_section(list: &SongCarousel, more: &gtk::Button) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_bottom(12)
        .visible(false)
        .build();
    let title_row = heading_row(&gettext("Top Songs"));
    title_row.append(more);
    section.append(&title_row);
    section.append(list.widget());
    section
}

/// Joins the genres, the origin, and the start year in one line, for
/// example "Alternative · Abingdon, England · Formed in 1985". It skips the
/// missing parts.
fn details_line(data: &ArtistData) -> String {
    let genres = (!data.genres.is_empty()).then(|| data.genres.join(", "));
    let start = data.start.as_ref().map(|start| match start {
        ArtistStart::Born(year) => gettext("Born in {year}").replace("{year}", year),
        ArtistStart::Formed(year) => gettext("Formed in {year}").replace("{year}", year),
    });
    let parts: Vec<String> = [genres, data.origin.clone(), start]
        .into_iter()
        .flatten()
        .collect();
    parts.join(" · ")
}

/// Builds the hidden Info button. `show` shows it when the artist has
/// information.
fn build_info_button() -> gtk::Button {
    gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text(gettext("Artist Information"))
        .valign(gtk::Align::Center)
        .css_classes(["boxy"])
        .visible(false)
        .build()
}

/// Builds the scrolling content: the header, the latest release, the top
/// songs, and the other rows, like in Apple Music.
fn build_content(header: &DetailHeader, songs: &gtk::Box, rows: &[ListRow]) -> gtk::ScrolledWindow {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_bottom(18)
        .build();
    page.append(header.widget());
    for row in rows {
        page.append(row.tiles.widget());
        if row.list == ArtistList::LatestRelease {
            page.append(songs);
        }
    }
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&page)
        .build()
}
