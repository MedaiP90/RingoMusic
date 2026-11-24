// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Dialog with all the information about an artist: the photo, the name, the
//! genres, the origin, the start year, and the biography.

use gettextrs::{gettext, ngettext};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::pango::WrapMode;

use crate::detail::{ArtistData, ArtistStart};

/// Size of the photo in pixels.
const PHOTO_SIZE: i32 = 160;

/// Space around and between the parts of the dialog, in pixels.
const SPACING: i32 = 18;

/// Shows the dialog for the artist in `data` over the window of `parent`.
/// `photo` is the photo that the artist page shows, so the dialog does not
/// load it again. The dialog takes the tint of the artist page.
pub fn present(parent: &impl IsA<gtk::Widget>, data: &ArtistData, photo: &gtk::Image) {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    content.append(&build_photo(photo));
    content.append(&build_name(&data.name));
    if let Some(facts) = build_facts(data) {
        content.append(&facts);
    }
    if let Some(biography) = &data.biography {
        content.append(&build_biography(biography));
    }
    let toolbar = adw::ToolbarView::builder()
        .css_classes(["album-tint"])
        .content(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_height(true)
                .child(&content)
                .build(),
        )
        .build();
    toolbar.add_top_bar(&adw::HeaderBar::builder().css_classes(["flat"]).build());
    adw::Dialog::builder()
        .title(&data.name)
        .content_width(480)
        .content_height(640)
        .child(&toolbar)
        .build()
        .present(Some(parent));
}

/// A round copy of the photo of the artist page. It has the same style as
/// the artwork of the pages.
fn build_photo(photo: &gtk::Image) -> gtk::Image {
    let image = gtk::Image::builder()
        .pixel_size(PHOTO_SIZE)
        .halign(gtk::Align::Center)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["card", "circular"])
        .build();
    match photo.paintable() {
        Some(paintable) => image.set_paintable(Some(&paintable)),
        None => image.set_icon_name(photo.icon_name().as_deref()),
    }
    image
}

fn build_name(name: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(name)
        .wrap(true)
        .wrap_mode(WrapMode::WordChar)
        .justify(gtk::Justification::Center)
        .css_classes(["title-1"])
        .build()
}

/// A list with one row for each known fact. `None` if no fact is known.
fn build_facts(data: &ArtistData) -> Option<adw::PreferencesGroup> {
    let genre_count = u32::try_from(data.genres.len()).unwrap_or(u32::MAX);
    let genres = (!data.genres.is_empty()).then(|| {
        (
            ngettext("Genre", "Genres", genre_count),
            data.genres.join(", "),
        )
    });
    let origin = data
        .origin
        .clone()
        .map(|origin| (gettext("Origin"), origin));
    let start = data.start.as_ref().map(|start| match start {
        ArtistStart::Born(year) => (gettext("Born"), year.clone()),
        ArtistStart::Formed(year) => (gettext("Formed"), year.clone()),
    });
    let facts: Vec<(String, String)> = [genres, origin, start].into_iter().flatten().collect();
    if facts.is_empty() {
        return None;
    }
    let group = adw::PreferencesGroup::new();
    for (label, value) in facts {
        group.add(
            &adw::ActionRow::builder()
                .title(label)
                .subtitle(value)
                .use_markup(false)
                .css_classes(["property"])
                .build(),
        );
    }
    Some(group)
}

/// The heading and the full text of the biography.
fn build_biography(biography: &str) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    section.append(
        &gtk::Label::builder()
            .label(gettext("Biography"))
            .xalign(0.0)
            .css_classes(["title-4"])
            .build(),
    );
    section.append(
        &gtk::Label::builder()
            .label(biography)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(WrapMode::WordChar)
            .build(),
    );
    section
}
