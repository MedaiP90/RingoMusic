// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The message about the library functions that Apple does not offer to other apps.
//!
//! The first-start dialog and the Account page of the preferences use the same text.

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::glib;
use ringo_core::settings::Settings;
use tracing::{debug, warn};

/// Version of the list of unavailable functions. Increase it each time the
/// list changes. Then the dialog shows again, one time, for each user.
const LIBRARY_DISCLAIMER_VERSION: u32 = 2;

/// Address of the web player of Apple Music.
const MUSIC_SITE: &str = "https://music.apple.com";

/// Response ID of the button that accepts the message.
const ACCEPT_RESPONSE: &str = "accept";

/// Returns true if the user did not accept the current version of the message.
fn should_show(accepted_version: u32, current_version: u32) -> bool {
    accepted_version < current_version
}

/// Returns the text of the message, with one function on each line.
pub fn body() -> String {
    let unavailable = [
        gettext("Rename a playlist or change its description"),
        gettext("Delete a playlist"),
        gettext("Remove songs from a playlist"),
        gettext("Change the order of songs in a playlist"),
        gettext("Remove items from your library"),
        gettext("Remove artists from your favorites"),
    ]
    .map(|function| format!("• {function}"))
    .join("\n");
    [
        gettext(
            "RingoMusic respects Apple's terms of use. It uses only the functions that Apple makes available to other apps.",
        ),
        gettext("These functions are not available in RingoMusic:"),
        unavailable,
        gettext(
            "To do these actions, use the Apple Music app or music.apple.com. RingoMusic shows the changes the next time it loads your library.",
        ),
        gettext("RingoMusic is not affiliated with Apple."),
    ]
    .join("\n\n")
}

/// Opens music.apple.com in the default browser. This works in a Flatpak too.
pub fn open_music_site(widget: &impl IsA<gtk::Widget>) {
    let parent = widget.root().and_downcast::<gtk::Window>();
    let launcher = gtk::UriLauncher::new(MUSIC_SITE);
    glib::spawn_future_local(async move {
        if let Err(error) = launcher.launch_future(parent.as_ref()).await {
            debug!(?error, "cannot open music.apple.com");
        }
    });
}

/// Shows the dialog on top of `parent` if the user did not accept this version yet.
pub fn present_if_new(parent: &impl IsA<gtk::Widget>, settings: &Settings) {
    if !should_show(
        settings.library_disclaimer_version(),
        LIBRARY_DISCLAIMER_VERSION,
    ) {
        return;
    }
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Some Library Functions Are Not Available")),
        Some(&body()),
    );
    // A button in the dialog body keeps the dialog open. A response would close it.
    let link = gtk::Button::with_label(&gettext("Open music.apple.com"));
    link.set_halign(gtk::Align::Center);
    link.connect_clicked(open_music_site);
    dialog.set_extra_child(Some(&link));
    dialog.add_response(ACCEPT_RESPONSE, &gettext("I _Understand"));
    dialog.set_default_response(Some(ACCEPT_RESPONSE));
    dialog.set_close_response(ACCEPT_RESPONSE);
    let settings = settings.clone();
    dialog.connect_response(Some(ACCEPT_RESPONSE), move |_, _| {
        if let Err(error) = settings.set_library_disclaimer_version(LIBRARY_DISCLAIMER_VERSION) {
            warn!(%error, "cannot save the accepted disclaimer version");
        }
    });
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_start_shows_the_message() {
        assert!(should_show(0, 1));
    }

    #[test]
    fn accepted_version_hides_the_message() {
        assert!(!should_show(1, 1));
    }

    #[test]
    fn new_version_shows_the_message_again() {
        assert!(should_show(1, 2));
    }
}
