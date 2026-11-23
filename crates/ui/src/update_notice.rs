// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tells the user that a newer release exists.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::glib;
use relm4::prelude::*;
use ringo_core::update_checker::{self, Release, UpdateChecker};
use tracing::debug;

use crate::app::{App, Input};
use crate::services::AppServices;

/// Starts the update check if it is due. The result arrives as
/// [`Input::UpdateAvailable`].
pub fn start(services: &AppServices, sender: &ComponentSender<App>) {
    let settings = services.settings();
    let now = update_checker::unix_now();
    if !update_checker::should_check(
        settings.check_updates(),
        &services.paths().install_prefix,
        settings.last_update_check(),
        now,
    ) {
        return;
    }
    // Save the time before the request, so a failed request also waits 24 hours.
    if let Err(error) = settings.set_last_update_check(now) {
        debug!(?error, "cannot save the time of the update check");
    }
    let checker = match UpdateChecker::new(env!("CARGO_PKG_VERSION")) {
        Ok(checker) => checker,
        Err(error) => {
            debug!(?error, "cannot start the update check");
            return;
        }
    };
    let sender = sender.input_sender().clone();
    relm4::spawn(async move {
        if let Some(release) = checker.newer_release().await {
            // The window can be closed already. Then nobody needs the answer.
            let _ = sender.send(Input::UpdateAvailable(release));
        }
    });
}

/// Makes the toast that offers the release page.
pub fn toast(release: &Release, window: &adw::ApplicationWindow) -> adw::Toast {
    let title = gettext("Version {version} is available")
        .replace("{version}", &release.version.to_string());
    let toast = adw::Toast::builder()
        .title(title)
        .button_label(gettext("Download"))
        .timeout(0)
        .build();
    let url = release.url.clone();
    let window = window.clone();
    toast.connect_button_clicked(move |_| {
        let launcher = gtk::UriLauncher::new(&url);
        let window = window.clone();
        glib::spawn_future_local(async move {
            if let Err(error) = launcher.launch_future(Some(&window)).await {
                debug!(?error, "cannot open the release page");
            }
        });
    });
    toast
}
