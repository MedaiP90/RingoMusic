// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Accent color of the application. The value `system` keeps the accent color
//! of the desktop. Any other value overrides it with a fixed color.

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gdk;
use ringo_core::settings::Settings;

/// Key of the accent color in the settings.
const KEY: &str = "accent-color";

/// The value that follows the desktop.
const SYSTEM: &str = "system";

/// The values of the setting, in the order of the preferences list.
pub const IDS: [&str; 10] = [
    SYSTEM, "blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "slate",
];

/// Returns the name of the value `id` for the user. Each name is a literal, so
/// the translation tools find it.
pub fn label(id: &str) -> String {
    match id {
        "blue" => gettext("Blue"),
        "teal" => gettext("Teal"),
        "green" => gettext("Green"),
        "yellow" => gettext("Yellow"),
        "orange" => gettext("Orange"),
        "red" => gettext("Red"),
        "pink" => gettext("Pink"),
        "purple" => gettext("Purple"),
        "slate" => gettext("Slate"),
        _ => gettext("System"),
    }
}

/// Returns the position of `id` in [`IDS`]. An unknown value gives the system entry.
pub fn position(id: &str) -> u32 {
    IDS.iter()
        .position(|candidate| *candidate == id)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// Applies the accent color of the settings now, and each time that the
/// setting or the light and dark style changes. Call it one time at start.
pub fn install(settings: &Settings) {
    let provider = gtk::CssProvider::new();
    #[expect(deprecated, reason = "GTK has no other way to add a global style")]
    gtk::StyleContext::add_provider_for_display(
        &gdk::Display::default().expect("internal error: GTK has a display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let reload = {
        let (provider, settings) = (provider, settings.clone());
        move || provider.load_from_string(&css(&settings.accent_color()))
    };
    reload();
    let on_setting = reload.clone();
    settings
        .raw()
        .connect_changed(Some(KEY), move |_, _| on_setting());
    adw::StyleManager::default().connect_dark_notify(move |_| reload());
}

/// Returns the style that sets the accent color `id`. The system value gives an
/// empty style, so the style of the desktop stays.
fn css(id: &str) -> String {
    let Some(color) = accent(id) else {
        return String::new();
    };
    let dark = adw::StyleManager::default().is_dark();
    let background = color.to_rgba().to_str();
    let text = color.to_standalone_rgba(dark).to_str();
    format!(
        "@define-color accent_bg_color {background};
         @define-color accent_color {text};
         @define-color accent_fg_color white;
         :root {{
           --accent-bg-color: {background};
           --accent-color: {text};
           --accent-fg-color: white;
         }}"
    )
}

fn accent(id: &str) -> Option<adw::AccentColor> {
    Some(match id {
        "blue" => adw::AccentColor::Blue,
        "teal" => adw::AccentColor::Teal,
        "green" => adw::AccentColor::Green,
        "yellow" => adw::AccentColor::Yellow,
        "orange" => adw::AccentColor::Orange,
        "red" => adw::AccentColor::Red,
        "pink" => adw::AccentColor::Pink,
        "purple" => adw::AccentColor::Purple,
        "slate" => adw::AccentColor::Slate,
        _ => return None,
    })
}
