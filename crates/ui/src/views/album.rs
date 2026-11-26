// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Album page: the header, and the songs of each disc.

use std::sync::Arc;
use std::time::Duration;

use gettextrs::{gettext, ngettext};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use ringo_am_api::models::AlbumId;
use ringo_am_api::write::CatalogItem;
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::add_to_playlist::ItemMenu;
use crate::detail::{self, AlbumData, is_library_id};
use crate::item::SongItem;
use crate::item_actions::{self, OPEN_ARTIST, SongCatalog};
use crate::services::{ApiClient, AppServices};
use crate::tint;
use crate::views::detail_frame::DetailFrame;
use crate::views::phase::Phase;
use crate::widgets::detail_header::{ArtworkShape, BylineLink, DetailHeader, PlayActions};
use crate::widgets::song_list::SongList;
use crate::widgets::song_row::Lead;

/// Seconds in a minute.
const SECONDS_PER_MINUTE: u64 = 60;

/// What an album page needs to start.
pub struct AlbumInit {
    pub services: AppServices,
    pub catalog: SongCatalog,
    /// ID of a catalog album or a library album.
    pub id: String,
}

/// Input of an album page.
#[derive(Debug)]
pub enum AlbumInput {
    /// Loads the album again.
    Retry,
    /// A load ended. `None` is a failure.
    Loaded(Option<AlbumData>),
    /// The user pressed the song at the position in the album.
    PlaySong(usize),
    PlayAll,
    ShuffleAll,
}

/// An album page.
pub struct AlbumView {
    id: String,
    api: Arc<ApiClient>,
    core: CoreHandle,
    catalog: SongCatalog,
    title: String,
    phase: Phase,
    songs: Vec<SongItem>,
    header: DetailHeader,
    /// The menu of the page header bar. It adds the album to a playlist or to the library.
    add_menu: ItemMenu,
    frame: DetailFrame,
    actions: PlayActions,
    /// The box that holds the disc headings and the song lists.
    discs: gtk::Box,
    /// The song lists of the discs. They hold the rows.
    lists: Vec<SongList>,
    pending: Option<JoinHandle<()>>,
}

#[relm4::component(pub)]
impl SimpleComponent for AlbumView {
    type Init = AlbumInit;
    type Input = AlbumInput;
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
                    pack_end = &add_menu_widget.clone() -> gtk::MenuButton {},
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
        let AlbumInit {
            services,
            catalog,
            id,
        } = init;
        let header = DetailHeader::new(ArtworkShape::Square);
        let add_menu = ItemMenu::new();
        let add_menu_widget = add_menu.widget().clone();
        tint::follow_viewed(header.artwork_image(), &root, &id);
        let discs = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let content = build_content(&header, &discs);
        let input = sender.input_sender().clone();
        let frame = DetailFrame::new(&content, &gettext("This album has no songs"), move || {
            input.emit(AlbumInput::Retry);
        });
        let frame_widget = frame.widget().clone();
        let (play, shuffle) = (sender.input_sender().clone(), sender.input_sender().clone());
        let actions = PlayActions::install(
            &root,
            move || play.emit(AlbumInput::PlayAll),
            move || shuffle.emit(AlbumInput::ShuffleAll),
        );

        let mut model = Self {
            id,
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            title: gettext("Album"),
            phase: Phase::Idle,
            songs: Vec::new(),
            header,
            add_menu,
            frame,
            actions,
            discs,
            lists: Vec::new(),
            pending: None,
        };
        model.reload(&sender);
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AlbumInput::Retry => self.reload(&sender),
            AlbumInput::Loaded(Some(data)) => self.show(&data, &sender),
            AlbumInput::Loaded(None) => self.set_phase(Phase::Failed),
            AlbumInput::PlaySong(position) => {
                item_actions::play_songs(&self.core, &self.songs, position);
            }
            AlbumInput::PlayAll => item_actions::play_songs(&self.core, &self.songs, 0),
            AlbumInput::ShuffleAll => item_actions::shuffle_songs(&self.core, &self.songs),
        }
    }
}

impl AlbumView {
    fn reload(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.set_phase(Phase::Loading);
        let storefront = detail::storefront(&self.core.subscribe().borrow()).unwrap_or_default();
        let (api, id) = (Arc::clone(&self.api), self.id.clone());
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let data = match detail::load_album(&api, &storefront, &id).await {
                Ok(data) => Some(data),
                Err(error) => {
                    warn!(%error, id, "cannot load the album");
                    None
                }
            };
            input.emit(AlbumInput::Loaded(data));
        }));
    }

    fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.frame.show(phase);
        self.actions.set_enabled(phase == Phase::Content);
    }

    /// Puts the album in the widgets.
    fn show(&mut self, data: &AlbumData, sender: &ComponentSender<Self>) {
        self.title.clone_from(&data.title);
        self.songs = data.songs();
        self.catalog.remember(&self.songs);
        self.add_menu
            .set_songs(self.songs.iter().map(|song| song.id().to_owned()).collect());
        self.add_menu.set_library_item(
            (!is_library_id(&self.id)).then(|| CatalogItem::Album(AlbumId::from(self.id.clone()))),
        );
        self.header.set_artwork(data.artwork_template.as_deref());
        self.header.set_title(&data.title);
        let link = data.artist_id.as_deref().map(|target| BylineLink {
            action: OPEN_ARTIST,
            target,
        });
        self.header.set_byline(&data.artist, link);
        let details = details_line(
            data.year.as_deref(),
            data.tracks.len(),
            data.total_duration(),
        );
        self.header.set_details(&details);
        self.header.set_description(None);
        self.show_discs(data, sender);
        let has_songs = !self.songs.is_empty();
        self.set_phase(if has_songs {
            Phase::Content
        } else {
            Phase::Empty
        });
    }

    /// Builds one list for each disc. A heading names the disc when the album
    /// has more than one.
    fn show_discs(&mut self, data: &AlbumData, sender: &ComponentSender<Self>) {
        while let Some(child) = self.discs.first_child() {
            self.discs.remove(&child);
        }
        self.lists.clear();
        let groups = detail::discs(&data.tracks);
        let has_many_discs = groups.len() > 1;
        let mut first_position = 0;
        for group in groups {
            if has_many_discs {
                let number = group.first().map_or(0, |track| track.disc);
                self.discs.append(&disc_heading(number));
            }
            let input = sender.input_sender().clone();
            let list = SongList::new(move |position| {
                input.emit(AlbumInput::PlaySong(first_position + position));
            });
            list.set_songs(
                group
                    .iter()
                    .map(|track| (&track.song, Lead::TrackNumber(track.number))),
            );
            self.discs.append(list.widget());
            self.lists.push(list);
            first_position += group.len();
        }
    }
}

/// Builds the scrolling content: the header and the disc lists.
fn build_content(header: &DetailHeader, discs: &gtk::Box) -> gtk::ScrolledWindow {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.append(header.widget());
    discs.set_margin_bottom(18);
    page.append(discs);
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&page)
        .build()
}

fn disc_heading(number: u32) -> gtk::Label {
    gtk::Label::builder()
        .label(gettext("Disc {number}").replace("{number}", &number.to_string()))
        .xalign(0.0)
        .margin_start(18)
        .css_classes(["heading"])
        .build()
}

/// The line under the artist: the year, the song count, and the length.
fn details_line(year: Option<&str>, song_count: usize, total: Duration) -> String {
    let count = u32::try_from(song_count).unwrap_or(u32::MAX);
    let songs = ngettext("{count} song", "{count} songs", count)
        .replace("{count}", &song_count.to_string());
    let minutes = gettext("{minutes} min").replace(
        "{minutes}",
        &(total.as_secs() / SECONDS_PER_MINUTE).to_string(),
    );
    let parts: Vec<String> = year
        .map(str::to_owned)
        .into_iter()
        .chain([songs, minutes])
        .collect();
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_line_joins_the_year_the_songs_and_the_length() {
        let line = details_line(Some("1997"), 12, Duration::from_mins(50));
        assert_eq!(line, "1997 · 12 songs · 50 min");
    }

    #[test]
    fn details_line_skips_a_missing_year() {
        let line = details_line(None, 1, Duration::from_secs(90));
        assert_eq!(line, "1 song · 1 min");
    }
}
