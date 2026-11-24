// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! A page of titled sections. Each section is a row of tiles or a list of
//! songs. Home, New, and Made For You use it: each page gives a loader and its
//! texts.

// Some items are for the Made For You page.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the Made For You page uses them")
)]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use ringo_am_api::models::{Group, GroupKind, Resource};
use ringo_core::runner::CoreHandle;
use tokio::task::JoinHandle;
use tracing::debug;

use crate::detail::storefront;
use crate::item::{SongItem, TileItem};
use crate::item_actions::{self, SongCatalog};
use crate::services::{ApiClient, AppServices};
use crate::views::phase::Phase;
use crate::widgets::song_carousel::SongCarousel;
use crate::widgets::tile::{self, TileRow};

/// Maximum number of rows in a song section. It is the same limit as the
/// songs of the search page.
const SONG_LIMIT: usize = 25;

/// What a loader needs. Only `Send` data: the load runs on another thread.
pub struct LoadContext {
    pub api: Arc<ApiClient>,
    /// The storefront of the user. `None` if the user is not signed in.
    pub storefront: Option<String>,
}

/// The result of a load. `None` means that the request failed.
pub type SectionsFuture = Pin<Box<dyn Future<Output = Option<Vec<Section>>> + Send>>;

/// Gets the sections of a page.
pub type Loader = fn(LoadContext) -> SectionsFuture;

/// What the content of a section is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SectionContent {
    Tiles(Vec<TileItem>),
    Songs(Vec<SongItem>),
}

/// One titled section, ready for the widgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    pub title: String,
    pub content: SectionContent,
}

impl Section {
    /// A row of tiles. Resources without a tile are skipped. Returns `None`
    /// if no tile is left.
    pub fn tiles<'a>(
        title: String,
        resources: impl IntoIterator<Item = &'a Resource>,
    ) -> Option<Self> {
        let items: Vec<TileItem> = resources.into_iter().filter_map(tile_of).collect();
        (!items.is_empty()).then_some(Self {
            title,
            content: SectionContent::Tiles(items),
        })
    }

    /// A list of songs, cut at [`SONG_LIMIT`]. Other resources are skipped.
    /// Returns `None` if no song is left.
    pub fn songs<'a>(
        title: String,
        resources: impl IntoIterator<Item = &'a Resource>,
    ) -> Option<Self> {
        let items: Vec<SongItem> = resources
            .into_iter()
            .filter_map(song_of)
            .take(SONG_LIMIT)
            .collect();
        (!items.is_empty()).then_some(Self {
            title,
            content: SectionContent::Songs(items),
        })
    }

    /// Converts `group` as its kind says. A group without a title gets an
    /// empty one.
    pub fn from_group(group: &Group) -> Option<Self> {
        let title = group.title.clone().unwrap_or_default();
        match group.kind {
            GroupKind::TileRow => Self::tiles(title, &group.items),
            GroupKind::SongList => Self::songs(title, &group.items),
        }
    }
}

/// The tile of a catalog album, artist, or playlist. Other types have none.
pub fn tile_of(resource: &Resource) -> Option<TileItem> {
    match resource {
        Resource::Album(album) => Some(TileItem::from(album)),
        Resource::Artist(artist) => Some(TileItem::from(artist)),
        Resource::Playlist(playlist) => Some(TileItem::from(playlist)),
        Resource::Song(_)
        | Resource::LibrarySong(_)
        | Resource::LibraryAlbum(_)
        | Resource::LibraryArtist(_)
        | Resource::LibraryPlaylist(_)
        | Resource::Unknown => {
            debug!(?resource, "skipped a resource without a tile");
            None
        }
    }
}

/// The song of a catalog song or a library song. Other types have none.
fn song_of(resource: &Resource) -> Option<SongItem> {
    match resource {
        Resource::Song(song) => Some(SongItem::from(song)),
        Resource::LibrarySong(song) => Some(SongItem::from(song)),
        Resource::Album(_)
        | Resource::Artist(_)
        | Resource::Playlist(_)
        | Resource::LibraryAlbum(_)
        | Resource::LibraryArtist(_)
        | Resource::LibraryPlaylist(_)
        | Resource::Unknown => {
            debug!(?resource, "skipped a resource without a song");
            None
        }
    }
}

/// What the sections view needs to start.
pub struct SectionsInit {
    pub services: AppServices,
    pub catalog: SongCatalog,
    pub loader: Loader,
    /// Icon of the page that shows when there are no sections.
    pub empty_icon: &'static str,
    pub empty_title: String,
    pub empty_description: String,
    /// Title of the page that shows when the load fails.
    pub error_title: String,
}

/// Input of the sections view.
#[derive(Debug)]
pub enum SectionsInput {
    /// The page is on screen. The first call loads the sections.
    Shown,
    /// Loads the sections again.
    Refresh,
    /// A load ended. The number tells which load it was.
    Loaded(u64, Option<Vec<Section>>),
    /// The user activated a song: the position of the section, then the row.
    SongActivated(usize, usize),
}

/// A section on screen. The widgets own their images, so they live as long as
/// the page shows them.
enum Shown {
    Tiles(
        #[cfg_attr(
            test,
            expect(
                dead_code,
                reason = "the row owns the images that load in the background"
            )
        )]
        TileRow,
    ),
    Songs {
        #[cfg_attr(
            test,
            expect(
                dead_code,
                reason = "the carousel owns the images that load in the background"
            )
        )]
        carousel: SongCarousel,
        items: Vec<SongItem>,
    },
}

/// A page of titled sections.
pub struct SectionsView {
    api: Arc<ApiClient>,
    core: CoreHandle,
    catalog: SongCatalog,
    loader: Loader,
    empty_icon: &'static str,
    empty_title: String,
    empty_description: String,
    error_title: String,
    phase: Phase,
    /// Number of the last load. The result of an older load is dropped.
    generation: u64,
    pending: Option<JoinHandle<()>>,
    container: gtk::Box,
    shown: Vec<Shown>,
}

#[relm4::component(pub)]
impl SimpleComponent for SectionsView {
    type Init = SectionsInit;
    type Input = SectionsInput;
    type Output = ();

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                pack_end = &gtk::Button {
                    set_icon_name: "view-refresh-symbolic",
                    set_tooltip_text: Some(&gettext("Refresh (F5)")),
                    set_action_name: Some("win.refresh"),
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
                    set_icon_name: Some(model.empty_icon),
                    set_title: &model.empty_title,
                    set_description: Some(&model.empty_description),
                },

                add_named[Some(Phase::Failed.page())] = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &model.error_title,
                    set_description: Some(&gettext("Check your connection and try again.")),

                    #[wrap(Some)]
                    set_child = &gtk::Button {
                        set_label: &gettext("Try Again"),
                        set_halign: gtk::Align::Center,
                        add_css_class: "pill",
                        add_css_class: "suggested-action",
                        connect_clicked => SectionsInput::Refresh,
                    },
                },

                add_named[Some(Phase::Content.page())] = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_child: Some(&container),
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let SectionsInit {
            services,
            catalog,
            loader,
            empty_icon,
            empty_title,
            empty_description,
            error_title,
        } = init;
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        // The page can show again later. Only the first show loads.
        let input = sender.input_sender().clone();
        root.connect_map(move |_| input.emit(SectionsInput::Shown));

        let model = Self {
            api: Arc::clone(services.api()),
            core: services.core().clone(),
            catalog,
            loader,
            empty_icon,
            empty_title,
            empty_description,
            error_title,
            phase: Phase::Idle,
            generation: 0,
            pending: None,
            container: container.clone(),
            shown: Vec::new(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SectionsInput::Shown => {
                if self.phase == Phase::Idle {
                    self.load(&sender);
                }
            }
            SectionsInput::Refresh => self.load(&sender),
            SectionsInput::Loaded(generation, sections) => {
                if generation == self.generation {
                    self.finish(sections, &sender);
                }
            }
            SectionsInput::SongActivated(section, position) => {
                if let Some(Shown::Songs { items, .. }) = self.shown.get(section) {
                    item_actions::play_songs(&self.core, items, position);
                }
            }
        }
    }
}

impl SectionsView {
    /// Starts a new request and cancels the one that runs.
    fn load(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.generation += 1;
        self.phase = Phase::Loading;
        let context = LoadContext {
            api: Arc::clone(&self.api),
            storefront: storefront(&self.core.subscribe().borrow()),
        };
        let (loader, generation) = (self.loader, self.generation);
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let sections = loader(context).await;
            input.emit(SectionsInput::Loaded(generation, sections));
        }));
    }

    /// Shows the end of a load: the sections, or the failure.
    fn finish(&mut self, sections: Option<Vec<Section>>, sender: &ComponentSender<Self>) {
        let Some(sections) = sections else {
            self.phase = Phase::Failed;
            return;
        };
        self.phase = if sections.is_empty() {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.show(&sections, sender);
    }

    /// Replaces the content with one widget for each section.
    fn show(&mut self, sections: &[Section], sender: &ComponentSender<Self>) {
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        self.shown = sections
            .iter()
            .enumerate()
            .map(|(index, section)| self.build(index, section, sender))
            .collect();
    }

    /// Builds the widget of one section and adds it to the page.
    fn build(&self, index: usize, section: &Section, sender: &ComponentSender<Self>) -> Shown {
        match &section.content {
            SectionContent::Tiles(items) => {
                let row = TileRow::new(&section.title);
                row.set_items(items);
                self.container.append(row.widget());
                Shown::Tiles(row)
            }
            SectionContent::Songs(items) => {
                self.catalog.remember(items);
                let input = sender.input_sender().clone();
                let carousel = SongCarousel::new(move |position| {
                    input.emit(SectionsInput::SongActivated(index, position));
                });
                carousel.set_songs(items);
                self.container
                    .append(&songs_section(&section.title, &carousel));
                Shown::Songs {
                    carousel,
                    items: items.clone(),
                }
            }
        }
    }
}

/// Builds the heading with the song carousel under it.
fn songs_section(title: &str, carousel: &SongCarousel) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    section.append(&tile::heading(title));
    section.append(carousel.widget());
    section
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn resource(kind: &str, id: &str) -> Resource {
        serde_json::from_value(json!({ "id": id, "type": kind, "attributes": { "name": id } }))
            .expect("the test resource is valid")
    }

    fn song_group(count: usize) -> Group {
        Group {
            title: Some("New Music".to_owned()),
            kind: GroupKind::SongList,
            items: (0..count)
                .map(|number| resource("songs", &number.to_string()))
                .collect(),
        }
    }

    #[test]
    fn song_group_keeps_the_title_and_the_order_of_the_songs() {
        let section = Section::from_group(&song_group(2)).expect("the group has songs");
        let SectionContent::Songs(songs) = &section.content else {
            panic!("a song group must give a song section");
        };
        let ids: Vec<&str> = songs.iter().map(SongItem::id).collect();
        assert_eq!((section.title.as_str(), ids), ("New Music", vec!["0", "1"]));
    }

    #[test]
    fn song_group_is_cut_at_the_limit() {
        let section =
            Section::from_group(&song_group(SONG_LIMIT + 5)).expect("the group has songs");
        let SectionContent::Songs(songs) = &section.content else {
            panic!("a song group must give a song section");
        };
        assert_eq!(songs.len(), SONG_LIMIT);
    }

    #[test]
    fn song_section_skips_resources_without_a_song() {
        let resources = [resource("albums", "a"), resource("songs", "s")];
        let section = Section::songs(String::new(), &resources).expect("one song is left");
        assert_eq!(
            section.content,
            Section::from_group(&Group {
                title: None,
                kind: GroupKind::SongList,
                items: vec![resource("songs", "s")],
            })
            .expect("the group has a song")
            .content
        );
    }

    #[test]
    fn tile_group_gives_a_tile_section() {
        let group = Group {
            title: None,
            kind: GroupKind::TileRow,
            items: vec![resource("albums", "a")],
        };
        let section = Section::from_group(&group).expect("the group has a tile");
        assert!(matches!(section.content, SectionContent::Tiles(ref tiles) if tiles.len() == 1));
    }

    #[test]
    fn song_section_without_songs_is_dropped() {
        let resources = [resource("albums", "a")];
        assert_eq!(Section::songs(String::new(), &resources), None);
    }
}
