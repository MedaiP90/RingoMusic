// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The list of sections in the sidebar.

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::prelude::*;

use crate::section::Section;

/// Builds the list with one row for each section. The row order is the order
/// of [`Section::ALL`]. The "Library" heading is a header, not a row.
pub fn build_list() -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["navigation-sidebar"])
        .build();
    for section in Section::ALL {
        list.append(&build_row(section));
    }
    list.set_header_func(|row, _previous| {
        let starts_library = Section::from_row_index(row.index()) == Some(Section::Playlists);
        if starts_library && row.header().is_none() {
            let heading = gtk::Label::builder()
                .label(gettext("Library"))
                .halign(gtk::Align::Start)
                .margin_top(12)
                .margin_start(12)
                .css_classes(["heading", "dim-label"])
                .build();
            row.set_header(Some(&heading));
        }
    });
    list
}

/// Builds a row with the icon and the name of `section`.
fn build_row(section: Section) -> gtk::ListBoxRow {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    content.append(&gtk::Image::from_icon_name(section.icon_name()));
    content.append(
        &gtk::Label::builder()
            .label(section.title())
            .halign(gtk::Align::Start)
            .build(),
    );
    gtk::ListBoxRow::builder().child(&content).build()
}
