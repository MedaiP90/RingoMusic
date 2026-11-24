// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The right pane of the library artists page. It shows the albums in the
//! library of the selected artist, and a button that opens the full artist
//! page.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::glib;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::detail;
use crate::item::{LibraryArtistItem, TileItem, TileKind};
use crate::services::{ApiClient, AppServices};
use crate::views::detail_frame;
use crate::views::phase::Phase;
use crate::widgets::library_tile::GridTile;

/// Name of the stack page while no artist is selected.
const NO_ARTIST_PAGE: &str = "no-artist";

/// What the pane needs to start.
pub struct ArtistAlbumsInit {
    pub services: AppServices,
    /// The split view that holds the pane. The pane shows itself in it.
    pub split_view: adw::NavigationSplitView,
}

/// Input of the pane.
#[derive(Debug)]
pub enum ArtistAlbumsInput {
    /// Shows the albums of the artist.
    Show(LibraryArtistItem),
    /// Loads the albums again after a failure.
    Retry,
    /// A load ended. The number tells which load it was.
    Loaded(u64, Option<Vec<TileItem>>),
    /// The user activated the album at the position.
    AlbumActivated(u32),
    /// The user asked for the full artist page.
    OpenArtist,
}

/// The albums of one library artist.
pub struct ArtistAlbumsView {
    api: Arc<ApiClient>,
    split_view: adw::NavigationSplitView,
    artist: Option<LibraryArtistItem>,
    phase: Phase,
    /// Number of the last load. A late result of an older load is ignored.
    generation: u64,
    pending: Option<JoinHandle<()>>,
    grid: TypedGridView<GridTile, gtk::NoSelection>,
    albums: Vec<TileItem>,
}

#[relm4::component(pub)]
impl SimpleComponent for ArtistAlbumsView {
    type Init = ArtistAlbumsInit;
    type Input = ArtistAlbumsInput;
    type Output = ();

    view! {
        adw::NavigationPage {
            #[watch]
            set_title: &model.title(),

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                append = &gtk::Box {
                    set_spacing: 12,
                    set_margin_top: 12,
                    set_margin_bottom: 6,
                    set_margin_start: 18,
                    set_margin_end: 18,
                    #[watch]
                    set_visible: model.artist.is_some(),

                    // Only a collapsed split view needs it.
                    #[name = "back_button"]
                    append = &gtk::Button {
                        set_icon_name: "go-previous-symbolic",
                        set_tooltip_text: Some(&gettext("Back")),
                        set_valign: gtk::Align::Center,
                        add_css_class: "flat",
                    },

                    append = &gtk::Label {
                        set_hexpand: true,
                        set_xalign: 0.0,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        add_css_class: "title-1",
                        #[watch]
                        set_label: &model.title(),
                    },

                    append = &gtk::Button {
                        set_label: &gettext("Open Artist Page"),
                        add_css_class: "boxy",
                        set_valign: gtk::Align::Center,
                        connect_clicked => ArtistAlbumsInput::OpenArtist,
                    },
                },

                append = &gtk::Stack {
                    set_vexpand: true,
                    #[watch]
                    set_visible_child_name: model.stack_page(),

                    add_named[Some(NO_ARTIST_PAGE)] = &adw::StatusPage {
                        set_icon_name: Some("avatar-default-symbolic"),
                        set_title: &gettext("No Artist Selected"),
                        set_description: Some(&gettext("Select an artist to see the albums in your library.")),
                    },

                    add_named[Some(Phase::Loading.page())] = &detail_frame::loading_spinner() -> adw::Spinner {},

                    add_named[Some(Phase::Empty.page())] = &adw::StatusPage {
                        set_icon_name: Some("media-optical-symbolic"),
                        set_title: &gettext("No Albums"),
                        set_description: Some(&gettext("Your library has no albums of this artist.")),
                    },

                    add_named[Some(Phase::Failed.page())] = &failed_status.clone() -> adw::StatusPage {},

                    add_named[Some(Phase::Content.page())] = &gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_child: Some(&model.grid.view),
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
        let ArtistAlbumsInit {
            services,
            split_view,
        } = init;
        let grid = TypedGridView::<GridTile, gtk::NoSelection>::new();
        grid.view.set_single_click_activate(true);
        let input = sender.input_sender().clone();
        grid.view.connect_activate(move |_, position| {
            input.emit(ArtistAlbumsInput::AlbumActivated(position));
        });

        let model = Self {
            api: Arc::clone(services.api()),
            split_view,
            artist: None,
            phase: Phase::Idle,
            generation: 0,
            pending: None,
            grid,
            albums: Vec::new(),
        };
        let input = sender.input_sender().clone();
        let failed_status = detail_frame::failed_page(&gettext("Cannot Load Albums"), move || {
            input.emit(ArtistAlbumsInput::Retry);
        });
        let widgets = view_output!();
        connect_back_button(&widgets.back_button, &model.split_view);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ArtistAlbumsInput::Show(artist) => self.show(artist, &sender),
            ArtistAlbumsInput::Retry => self.reload(&sender),
            ArtistAlbumsInput::Loaded(generation, albums) => self.finish(generation, albums),
            ArtistAlbumsInput::AlbumActivated(position) => {
                let index = usize::try_from(position).unwrap_or(usize::MAX);
                if let Some(album) = self.albums.get(index) {
                    self.open(TileKind::Album, &album.id);
                }
            }
            ArtistAlbumsInput::OpenArtist => {
                if let Some(artist) = &self.artist {
                    self.open(TileKind::Artist, artist.page_id());
                }
            }
        }
    }
}

impl ArtistAlbumsView {
    /// The name of the selected artist, or the page name if none is selected.
    fn title(&self) -> String {
        self.artist
            .as_ref()
            .map_or_else(|| gettext("Albums"), |artist| artist.tile.title.clone())
    }

    fn stack_page(&self) -> &'static str {
        if self.artist.is_some() {
            self.phase.page()
        } else {
            NO_ARTIST_PAGE
        }
    }

    /// Shows the pane with the albums of `artist`. The albums of the artist
    /// that the pane already shows load only again after a failure.
    fn show(&mut self, artist: LibraryArtistItem, sender: &ComponentSender<Self>) {
        let same = self.artist.as_ref() == Some(&artist);
        self.split_view.set_show_content(true);
        if same && self.phase != Phase::Failed {
            return;
        }
        self.artist = Some(artist);
        self.reload(sender);
    }

    /// Clears the grid and loads the albums of the selected artist.
    fn reload(&mut self, sender: &ComponentSender<Self>) {
        let Some(artist) = &self.artist else {
            return;
        };
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.generation += 1;
        self.grid.clear();
        self.albums.clear();
        self.phase = Phase::Loading;
        let (api, id, generation) = (
            Arc::clone(&self.api),
            artist.tile.id.clone(),
            self.generation,
        );
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let albums = match detail::load_library_artist_albums(&api, &id).await {
                Ok(albums) => Some(albums),
                Err(error) => {
                    warn!(%error, id, "cannot load the albums of a library artist");
                    None
                }
            };
            input.emit(ArtistAlbumsInput::Loaded(generation, albums));
        }));
    }

    /// Shows the result of the load `generation`, if it is the last load.
    fn finish(&mut self, generation: u64, albums: Option<Vec<TileItem>>) {
        if generation != self.generation {
            return;
        }
        self.pending = None;
        let Some(albums) = albums else {
            self.phase = Phase::Failed;
            return;
        };
        self.grid
            .extend_from_iter(albums.iter().cloned().map(GridTile::new));
        self.phase = if albums.is_empty() {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.albums = albums;
    }

    /// Opens the page of the item with the window action of `kind`.
    fn open(&self, kind: TileKind, id: &str) {
        if let Err(error) = self
            .grid
            .view
            .activate_action(kind.open_action(), Some(&id.to_variant()))
        {
            warn!(%error, "cannot open the page from the artist albums");
        }
    }
}

/// Makes the back button go back to the artist list. The button shows only
/// while the split view is collapsed.
fn connect_back_button(button: &gtk::Button, split_view: &adw::NavigationSplitView) {
    split_view
        .bind_property("collapsed", button, "visible")
        .sync_create()
        .build();
    button.connect_clicked(glib::clone!(
        #[weak]
        split_view,
        move |_| split_view.set_show_content(false)
    ));
}
