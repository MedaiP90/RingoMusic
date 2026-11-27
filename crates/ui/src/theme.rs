// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Light or dark style of the application. The value `system` follows the desktop.

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use ringo_core::settings::Settings;

/// Key of the color scheme in the settings.
const KEY: &str = "color-scheme";

/// The values of the setting, in the order of the preferences list.
pub const IDS: [&str; 3] = ["system", "light", "dark"];

/// Returns the name of the value `id` for the user.
pub fn label(id: &str) -> String {
    match id {
        "light" => gettext("Light"),
        "dark" => gettext("Dark"),
        _ => gettext("System"),
    }
}

/// Applies the color scheme of the settings now, and each time that the
/// setting changes. Call it one time at start.
pub fn install(settings: &Settings) {
    apply(&settings.color_scheme());
    let settings_for_change = settings.clone();
    settings.raw().connect_changed(Some(KEY), move |_, _| {
        apply(&settings_for_change.color_scheme());
    });
}

fn apply(id: &str) {
    let scheme = match id {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}
