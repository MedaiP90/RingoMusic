// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Made For You page: the personal mixes of the user, as groups of tiles.
//! The shared sections view shows them.

use gettextrs::gettext;
use ringo_am_api::endpoints;
use tracing::warn;

use crate::item_actions::SongCatalog;
use crate::services::AppServices;
use crate::views::sections::{LoadContext, Section, SectionsFuture, SectionsInit};

/// The config of the sections view for the Made For You page.
pub fn init(services: AppServices, catalog: SongCatalog) -> SectionsInit {
    SectionsInit {
        services,
        catalog,
        loader: load,
        empty_icon: "emblem-favorite-symbolic",
        empty_title: gettext("No Mixes Yet"),
        empty_description: gettext("Listen to more music, and Apple Music makes mixes for you."),
        error_title: gettext("Cannot Load Your Mixes"),
    }
}

fn load(context: LoadContext) -> SectionsFuture {
    Box::pin(async move {
        match endpoints::made_for_you(&*context.api).await {
            Ok(groups) => Some(groups.iter().filter_map(Section::from_group).collect()),
            Err(error) => {
                warn!(%error, "cannot load the made for you mixes");
                None
            }
        }
    })
}
