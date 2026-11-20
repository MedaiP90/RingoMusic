// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The about dialog.

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;

use crate::APP_ID;

/// Shows the about dialog on top of `parent`.
pub fn present(parent: &impl IsA<gtk::Widget>) {
    let repository = env!("CARGO_PKG_REPOSITORY");
    let dialog = adw::AboutDialog::builder()
        .application_name("RingoMusic")
        .application_icon(APP_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name(gettext("RingoMusic contributors"))
        .website(repository)
        .issue_url(format!("{repository}/issues"))
        .license_type(gtk::License::Gpl30)
        .comments(gettext(
            "RingoMusic is not affiliated with Apple Inc. Apple Music is a trademark of Apple Inc.",
        ))
        .build();
    dialog.present(Some(parent));
}
