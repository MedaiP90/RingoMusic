// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Primary menu of the application. Each window header can show it.

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::gio::Menu;

/// Builds a button with the primary menu.
pub fn primary_button() -> gtk::MenuButton {
    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(gettext("Main menu"))
        .menu_model(&primary_menu())
        .primary(true)
        .build()
}

fn primary_menu() -> Menu {
    let menu = Menu::new();
    let preferences = Menu::new();
    preferences.append(Some(&gettext("_Preferences")), Some("app.preferences"));
    let application = Menu::new();
    application.append(Some(&gettext("_About RingoMusic")), Some("app.about"));
    application.append(Some(&gettext("_Quit")), Some("app.quit"));
    menu.append_section(None, &preferences);
    menu.append_section(None, &application);
    menu
}
