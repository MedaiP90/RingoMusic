// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Home page: the personal recommendations, one row of tiles for each group.

use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use ringo_am_api::endpoints;
use ringo_am_api::models::{Recommendation, Resource};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::item::TileItem;
use crate::services::{ApiClient, AppServices};
use crate::views::section_page;
use crate::widgets::tile::TileRow;

/// One recommendation, ready for a row of tiles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileGroup {
    title: String,
    items: Vec<TileItem>,
}

impl TileGroup {
    /// Converts `recommendation`. Resources without a tile are skipped.
    /// Returns `None` if no tile is left.
    fn of(recommendation: &Recommendation) -> Option<Self> {
        let items: Vec<TileItem> = recommendation.contents.iter().filter_map(tile_of).collect();
        if items.is_empty() {
            return None;
        }
        let title = recommendation
            .title
            .clone()
            .unwrap_or_else(|| gettext("Recommended for You"));
        Some(Self { title, items })
    }
}

/// The tile of a catalog album, artist, or playlist. Other types have none.
fn tile_of(resource: &Resource) -> Option<TileItem> {
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
            debug!(?resource, "skipped a recommended resource without a tile");
            None
        }
    }
}

/// What the view shows.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Phase {
    /// The page was not shown yet. It looks like `Loading`.
    Idle,
    Loading,
    Empty,
    Failed,
    Content,
}

impl Phase {
    /// Name of the stack page.
    fn page(self) -> &'static str {
        match self {
            Self::Idle | Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Failed => "failed",
            Self::Content => "content",
        }
    }
}

/// What the home view needs to start.
pub struct HomeInit {
    pub services: AppServices,
    /// The back button of the header bar goes to the sidebar of this view.
    pub split_view: adw::NavigationSplitView,
}

/// Input of the home view.
#[derive(Debug)]
pub enum HomeInput {
    /// The page is on screen. The first call loads the recommendations.
    Shown,
    /// Loads the recommendations again.
    Refresh,
    /// A load ended. The number tells which load it was.
    Loaded(u64, Option<Vec<TileGroup>>),
}

/// The home page.
pub struct HomeView {
    api: Arc<ApiClient>,
    phase: Phase,
    /// Number of the last load. The result of an older load is dropped.
    generation: u64,
    pending: Option<JoinHandle<()>>,
    groups: gtk::Box,
    /// The rows own their images, so they live as long as the page shows them.
    rows: Vec<TileRow>,
}

#[relm4::component(pub)]
impl SimpleComponent for HomeView {
    type Init = HomeInit;
    type Input = HomeInput;
    type Output = ();

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                pack_start: &back_button,

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
                    set_icon_name: Some("user-home-symbolic"),
                    set_title: &gettext("No Recommendations Yet"),
                    set_description: Some(&gettext("Play some music and check back later.")),
                },

                add_named[Some(Phase::Failed.page())] = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &gettext("Cannot Load Recommendations"),
                    set_description: Some(&gettext("Check your connection and try again.")),

                    #[wrap(Some)]
                    set_child = &gtk::Button {
                        set_label: &gettext("Try Again"),
                        set_halign: gtk::Align::Center,
                        add_css_class: "pill",
                        add_css_class: "suggested-action",
                        connect_clicked => HomeInput::Refresh,
                    },
                },

                add_named[Some(Phase::Content.page())] = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_child: Some(&groups),
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let HomeInit {
            services,
            split_view,
        } = init;
        let back_button = section_page::back_button(&split_view);
        let groups = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        // The page can show again later. Only the first show loads.
        let input = sender.input_sender().clone();
        root.connect_map(move |_| input.emit(HomeInput::Shown));

        let model = Self {
            api: Arc::clone(services.api()),
            phase: Phase::Idle,
            generation: 0,
            pending: None,
            groups: groups.clone(),
            rows: Vec::new(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            HomeInput::Shown => {
                if self.phase == Phase::Idle {
                    self.load(&sender);
                }
            }
            HomeInput::Refresh => self.load(&sender),
            HomeInput::Loaded(generation, groups) => {
                if generation == self.generation {
                    self.finish(groups);
                }
            }
        }
    }
}

impl HomeView {
    /// Starts a new request and cancels the one that runs.
    fn load(&mut self, sender: &ComponentSender<Self>) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
        self.generation += 1;
        self.phase = Phase::Loading;
        let (api, generation) = (Arc::clone(&self.api), self.generation);
        let input = sender.input_sender().clone();
        self.pending = Some(relm4::spawn(async move {
            let groups = fetch(&api).await;
            input.emit(HomeInput::Loaded(generation, groups));
        }));
    }

    /// Shows the end of a load: the rows, or the failure.
    fn finish(&mut self, groups: Option<Vec<TileGroup>>) {
        let Some(groups) = groups else {
            self.phase = Phase::Failed;
            return;
        };
        self.phase = if groups.is_empty() {
            Phase::Empty
        } else {
            Phase::Content
        };
        self.show(&groups);
    }

    /// Replaces the rows with one row for each group.
    fn show(&mut self, groups: &[TileGroup]) {
        while let Some(row) = self.groups.first_child() {
            self.groups.remove(&row);
        }
        self.rows = groups
            .iter()
            .map(|group| {
                let row = TileRow::new(&group.title);
                row.set_items(&group.items);
                self.groups.append(row.widget());
                row
            })
            .collect();
    }
}

/// Gets the recommendations. Returns `None` if the request fails.
async fn fetch(api: &ApiClient) -> Option<Vec<TileGroup>> {
    match endpoints::recommendations(api).await {
        Ok(recommendations) => Some(groups_of(&recommendations)),
        Err(error) => {
            warn!(%error, "cannot load the recommendations");
            None
        }
    }
}

/// The groups of `recommendations`, in the same order. A recommendation
/// without tiles gives no group.
fn groups_of(recommendations: &[Recommendation]) -> Vec<TileGroup> {
    recommendations.iter().filter_map(TileGroup::of).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn recommendation(title: Option<&str>, contents: &serde_json::Value) -> Recommendation {
        let mut value = json!({
            "id": "r1",
            "type": "personal-recommendation",
            "relationships": { "contents": { "data": contents } }
        });
        if let Some(title) = title {
            value["attributes"] = json!({ "title": { "stringForDisplay": title } });
        }
        serde_json::from_value(value).expect("the test recommendation is valid")
    }

    fn album(id: &str) -> serde_json::Value {
        json!({ "id": id, "type": "albums", "attributes": { "name": id } })
    }

    fn song(id: &str) -> serde_json::Value {
        json!({ "id": id, "type": "songs", "attributes": { "name": id } })
    }

    #[test]
    fn group_keeps_the_title_and_the_order_of_the_tiles() {
        let group = TileGroup::of(&recommendation(
            Some("Made for You"),
            &json!([album("1"), album("2")]),
        ))
        .expect("the group has tiles");
        let ids: Vec<&str> = group.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(
            (group.title.as_str(), ids),
            ("Made for You", vec!["1", "2"])
        );
    }

    #[test]
    fn group_skips_resources_without_a_tile() {
        let contents = json!([song("1"), album("2"), { "id": "s", "type": "stations" }]);
        let group = TileGroup::of(&recommendation(Some("Mixed"), &contents))
            .expect("the group has one tile");
        assert_eq!(group.items.len(), 1);
    }

    #[test]
    fn recommendation_without_tiles_gives_no_group() {
        let groups = groups_of(&[recommendation(Some("Songs"), &json!([song("1")]))]);
        assert_eq!(groups, Vec::new());
    }

    #[test]
    fn group_without_a_title_gets_a_default_title() {
        let group = TileGroup::of(&recommendation(None, &json!([album("1")])))
            .expect("the group has tiles");
        assert!(!group.title.is_empty());
    }
}
