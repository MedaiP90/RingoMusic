// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Home page: the personal recommendations, one row of tiles for each group.
//! The shared sections view shows them.

use gettextrs::gettext;
use ringo_am_api::endpoints;
use ringo_am_api::models::Recommendation;
use tracing::warn;

use crate::item_actions::SongCatalog;
use crate::services::AppServices;
use crate::views::sections::{LoadContext, Section, SectionsFuture, SectionsInit};

/// The config of the sections view for the home page.
pub fn init(services: AppServices, catalog: SongCatalog) -> SectionsInit {
    SectionsInit {
        services,
        catalog,
        loader: load,
        empty_icon: "user-home-symbolic",
        empty_title: gettext("No Recommendations Yet"),
        empty_description: gettext("Play some music and check back later."),
        error_title: gettext("Cannot Load Recommendations"),
    }
}

fn load(context: LoadContext) -> SectionsFuture {
    Box::pin(async move {
        match endpoints::recommendations(&*context.api).await {
            Ok(recommendations) => Some(sections_of(&recommendations)),
            Err(error) => {
                warn!(%error, "cannot load the recommendations");
                None
            }
        }
    })
}

/// The row of tiles of `recommendation`. Resources without a tile are
/// skipped. Returns `None` if no tile is left.
fn section_of(recommendation: &Recommendation) -> Option<Section> {
    let title = recommendation
        .title
        .clone()
        .unwrap_or_else(|| gettext("Recommended for You"));
    Section::tiles(title, recommendation.contents.iter())
}

/// The sections of `recommendations`, in the same order. A recommendation
/// without tiles gives no section.
fn sections_of(recommendations: &[Recommendation]) -> Vec<Section> {
    recommendations.iter().filter_map(section_of).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::item::TileItem;
    use crate::views::sections::SectionContent;

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

    fn tiles(section: &Section) -> &[TileItem] {
        let SectionContent::Tiles(items) = &section.content else {
            panic!("a recommendation must give a tile section");
        };
        items
    }

    #[test]
    fn section_keeps_the_title_and_the_order_of_the_tiles() {
        let section = section_of(&recommendation(
            Some("Made for You"),
            &json!([album("1"), album("2")]),
        ))
        .expect("the section has tiles");
        let ids: Vec<&str> = tiles(&section)
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(
            (section.title.as_str(), ids),
            ("Made for You", vec!["1", "2"])
        );
    }

    #[test]
    fn section_skips_resources_without_a_tile() {
        let contents = json!([song("1"), album("2"), { "id": "s", "type": "stations" }]);
        let section = section_of(&recommendation(Some("Mixed"), &contents))
            .expect("the section has one tile");
        assert_eq!(tiles(&section).len(), 1);
    }

    #[test]
    fn recommendation_without_tiles_gives_no_section() {
        let sections = sections_of(&[recommendation(Some("Songs"), &json!([song("1")]))]);
        assert_eq!(sections, Vec::new());
    }

    #[test]
    fn section_without_a_title_gets_a_default_title() {
        let section =
            section_of(&recommendation(None, &json!([album("1")]))).expect("the section has tiles");
        assert_ne!(section.title, "");
    }
}
