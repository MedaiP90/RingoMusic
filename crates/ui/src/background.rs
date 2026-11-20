// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Background playback: what happens when the user closes the window.

use ashpd::desktop::background::Background;
use gettextrs::gettext;
use relm4::gtk::Application;
use relm4::gtk::gio::ApplicationHoldGuard;
use relm4::gtk::gio::prelude::ApplicationExtManual;
use ringo_core::paths::PackageFormat;
use ringo_core::settings::Settings;
use tracing::{info, warn};

/// What the application does when the window closes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CloseAction {
    /// Stop the application.
    Quit,
    /// Hide the window and keep playing.
    Hide,
    /// Ask the background portal first.
    RequestPermission,
    /// An answer from the portal is still to come. Do nothing.
    Wait,
}

/// State of the permission to run in the background.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Permission {
    /// Not asked yet.
    Unknown,
    /// The portal dialog is open.
    Requesting,
    /// The system lets the application run in the background.
    Granted,
}

/// Keeps the application alive after the window closes.
pub struct BackgroundPlayback {
    settings: Settings,
    permission: Permission,
    hold: Option<ApplicationHoldGuard>,
}

impl BackgroundPlayback {
    /// Native packages need no permission. Flatpak must ask the portal.
    pub fn new(settings: Settings, package: PackageFormat) -> Self {
        let permission = match package {
            PackageFormat::Native => Permission::Granted,
            PackageFormat::Flatpak => Permission::Unknown,
        };
        Self {
            settings,
            permission,
            hold: None,
        }
    }

    /// Decides what to do for a close request. A `RequestPermission` answer
    /// moves the state to "requesting".
    pub fn close_action(&mut self) -> CloseAction {
        if self.settings.quit_on_close() {
            return CloseAction::Quit;
        }
        match self.permission {
            Permission::Granted => CloseAction::Hide,
            Permission::Requesting => CloseAction::Wait,
            Permission::Unknown => {
                self.permission = Permission::Requesting;
                CloseAction::RequestPermission
            }
        }
    }

    /// Stores the answer of the portal.
    pub fn set_permission(&mut self, granted: bool) {
        self.permission = if granted {
            Permission::Granted
        } else {
            Permission::Unknown
        };
        if !granted {
            self.remember_refusal();
        }
    }

    /// Holds the application. A second call changes nothing.
    pub fn keep_alive(&mut self, app: &Application) {
        self.hold.get_or_insert_with(|| app.hold());
    }

    /// Returns the message for a refusal at the last run, and clears the flag.
    pub fn take_refusal_message(&self) -> Option<String> {
        if !self.settings.background_denied() {
            return None;
        }
        if let Err(error) = self.settings.set_background_denied(false) {
            warn!(%error, "cannot clear the background refusal flag");
        }
        Some(gettext(
            "The system did not allow background playback. RingoMusic stops when the window closes.",
        ))
    }

    /// Saves the refusal. The next start shows a message.
    fn remember_refusal(&self) {
        if let Err(error) = self.settings.set_background_denied(true) {
            warn!(%error, "cannot save the background refusal");
        }
    }
}

/// Asks the background portal for permission. Any failure counts as a refusal.
pub async fn request_permission() -> bool {
    // ponytail: no window identifier, so the portal dialog has no parent window.
    let response = Background::request()
        .reason(gettext("Continue playback when the window is closed").as_str())
        .send()
        .await
        .and_then(|request| request.response());
    match response {
        Ok(background) => background.run_in_background(),
        Err(error) => {
            info!(%error, "background portal request failed");
            false
        }
    }
}
