// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Banner that shows when the network is not available.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk::gio::NetworkMonitor;
use relm4::gtk::prelude::*;

/// Builds a banner that follows the network monitor. The banner does not block
/// the content below it.
pub fn build() -> adw::Banner {
    let banner = adw::Banner::new(&gettext(
        "No network connection. Some content is not available.",
    ));
    banner.add_css_class("offline");
    NetworkMonitor::default()
        .bind_property("network-available", &banner, "revealed")
        .sync_create()
        .invert_boolean()
        .build();
    banner
}
