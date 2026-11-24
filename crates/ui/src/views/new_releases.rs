// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! New page: the new releases of Apple Music, as groups of tiles and songs.
//! The shared sections view shows them.

use gettextrs::gettext;
use ringo_am_api::endpoints;
use tracing::warn;

use crate::item_actions::SongCatalog;
use crate::services::AppServices;
use crate::views::sections::{LoadContext, Section, SectionsFuture, SectionsInit};

/// The config of the sections view for the New page.
pub fn init(services: AppServices, catalog: SongCatalog) -> SectionsInit {
    SectionsInit {
        services,
        catalog,
        loader: load,
        empty_icon: "starred-symbolic",
        empty_title: gettext("No New Releases"),
        empty_description: gettext("Check back later."),
        error_title: gettext("Cannot Load New Releases"),
    }
}

fn load(context: LoadContext) -> SectionsFuture {
    Box::pin(async move {
        let Some(storefront) = context.storefront else {
            warn!("cannot load the new releases: the user is not signed in");
            return None;
        };
        match endpoints::new_releases(&*context.api, &storefront).await {
            Ok(groups) => Some(groups.iter().filter_map(Section::from_group).collect()),
            Err(error) => {
                warn!(%error, "cannot load the new releases");
                None
            }
        }
    })
}
