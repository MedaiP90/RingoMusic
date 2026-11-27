// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! "Add to Library" for catalog songs, albums, and playlists (decisions L5, L8, and L17).
//!
//! The write waits for the result. On success, the library views load again after a short delay,
//! because Apple can show the new item a little later.

use std::sync::Arc;
use std::time::Duration;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gio::ActionEntry;
use relm4::gtk::glib;
use ringo_am_api::write::{self, CatalogItem};
use tracing::warn;

use crate::library_write;
use crate::services;
use crate::views::library::LibraryInput;

/// Time between a successful write and the reload of the library views.
const RELOAD_DELAY: Duration = Duration::from_secs(2);

/// Name of the window action that marks the library views as out of date.
const INVALIDATE_ACTION_NAME: &str = "invalidate-library";

/// Adds the window action `win.invalidate-library`. The next show of a library page then loads
/// it again. A page that is not shown costs no request.
pub fn register(window: &adw::ApplicationWindow, pages: Vec<relm4::Sender<LibraryInput>>) {
    let invalidate = ActionEntry::builder(INVALIDATE_ACTION_NAME)
        .activate(move |_: &adw::ApplicationWindow, _, _| {
            for page in &pages {
                page.emit(LibraryInput::Invalidate);
            }
        })
        .build();
    window.add_action_entries([invalidate]);
}

/// Sends one request that adds `item` to the library. `anchor` is a widget of the view: the toast
/// appears over its window. On success, a toast tells it and the library views load again once,
/// after [`RELOAD_DELAY`].
pub fn add(anchor: &impl IsA<gtk::Widget>, item: CatalogItem) {
    let Some(services) = services::installed() else {
        warn!("the add to library item has no services");
        return;
    };
    let api = Arc::clone(services.api());
    let request = async move { write::add_to_library(api.as_ref(), &[item]).await };
    let toast_anchor: gtk::Widget = anchor.as_ref().clone();
    library_write::run(
        anchor,
        request,
        move |()| {
            library_write::notify(&toast_anchor, &gettext("Added to your library"));
            reload_later(&toast_anchor);
        },
        || {},
    );
}

/// Reads if `item` is in the library. The request is live. A failure counts as "not in the
/// library", so the item stays available (decision L17).
pub async fn is_in_library(item: CatalogItem) -> bool {
    let Some(services) = services::installed() else {
        return false;
    };
    let storefront = services.core().storefront().unwrap_or_default();
    let api = Arc::clone(services.api());
    let task =
        relm4::spawn(async move { write::library_status(api.as_ref(), &storefront, &item).await });
    match task.await {
        Ok(Ok(in_library)) => in_library,
        Ok(Err(error)) => {
            warn!(%error, "cannot read the library status");
            false
        }
        Err(join_error) => {
            warn!(%join_error, "the library status task did not finish");
            false
        }
    }
}

/// Runs the invalidate action on the window of `anchor` after [`RELOAD_DELAY`]. This is one
/// reload for one write, not a retry (rule R8). The window can close before the delay ends.
fn reload_later(anchor: &gtk::Widget) {
    let Some(root) = anchor.root() else {
        return;
    };
    let window = root.upcast::<gtk::Widget>().downgrade();
    glib::spawn_future_local(async move {
        glib::timeout_future(RELOAD_DELAY).await;
        let Some(window) = window.upgrade() else {
            return;
        };
        if let Err(error) = window.activate_action(&format!("win.{INVALIDATE_ACTION_NAME}"), None) {
            warn!(%error, "cannot reload the library views");
        }
    });
}
