// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Header of an album, artist, or playlist page: artwork, title, byline,
//! details, description, and the Play and Shuffle buttons.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::gio::{SimpleAction, SimpleActionGroup};
use relm4::gtk::pango::{EllipsizeMode, WrapMode};
use relm4::gtk::prelude::*;

use crate::widgets::artwork_image::ArtworkImage;

/// Size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 160;

/// Space between the artwork and the text, in pixels.
const SPACING: i32 = 18;

/// Width that the text needs before it moves under the artwork.
const TEXT_MIN_WIDTH: i32 = 240;

/// Lines of the description before the user expands it.
const COLLAPSED_LINES: i32 = 3;

/// Height limit of the expanded description, in pixels. A longer text scrolls.
const EXPANDED_MAX_HEIGHT: i32 = 160;

/// Text length from which the description gets the expand button.
const LONG_DESCRIPTION_CHARS: usize = 150;

/// Name of the action group that the page installs.
const ACTION_GROUP: &str = "detail";
const PLAY_ACTION: &str = "play";
const SHUFFLE_ACTION: &str = "shuffle";

/// The shape of the artwork.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArtworkShape {
    Square,
    /// For an artist photo.
    Round,
}

/// The link of the byline: an item action and the ID of its target.
#[derive(Copy, Clone, Debug)]
pub struct BylineLink<'a> {
    pub action: &'a str,
    pub target: &'a str,
}

/// The header widgets of a detail page.
pub struct DetailHeader {
    root: adw::WrapBox,
    artwork: ArtworkImage,
    title: gtk::Label,
    byline: gtk::Box,
    details: gtk::Label,
    description: Description,
    /// The row with the Play and Shuffle buttons.
    buttons: gtk::Box,
    shuffle_button: gtk::Button,
}

impl DetailHeader {
    /// Makes an empty header. The buttons run the actions that
    /// [`PlayActions::install`] adds to the page.
    pub fn new(shape: ArtworkShape) -> Self {
        let artwork = ArtworkImage::shared();
        artwork.set_artwork(None, ARTWORK_SIZE);
        artwork.widget().set_valign(gtk::Align::Start);
        if shape == ArtworkShape::Round {
            artwork.widget().add_css_class("circular");
        }
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(WrapMode::WordChar)
            .css_classes(["title-1"])
            .build();
        let byline = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let details = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let description = Description::new();
        let (buttons, shuffle_button) = build_buttons();

        let texts = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .width_request(TEXT_MIN_WIDTH)
            .build();
        texts.append(&title);
        texts.append(&byline);
        texts.append(&details);
        texts.append(description.widget());
        texts.append(&buttons);

        let root = adw::WrapBox::builder()
            .child_spacing(SPACING)
            .line_spacing(SPACING)
            .margin_top(SPACING)
            .margin_bottom(SPACING)
            .margin_start(SPACING)
            .margin_end(SPACING)
            .build();
        root.append(artwork.widget());
        root.append(&texts);
        Self {
            root,
            artwork,
            title,
            byline,
            details,
            description,
            buttons,
            shuffle_button,
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &adw::WrapBox {
        &self.root
    }

    /// The image of the artwork.
    pub fn artwork_image(&self) -> &gtk::Image {
        self.artwork.widget()
    }

    pub fn set_artwork(&self, template: Option<&str>) {
        self.artwork.set_artwork(template, ARTWORK_SIZE);
    }

    pub fn set_title(&self, title: &str) {
        self.title.set_label(title);
    }

    /// Shows `text` under the title. With a `link`, the text is a link
    /// button that runs the item action. Empty `text` hides the byline.
    pub fn set_byline(&self, text: &str, link: Option<BylineLink<'_>>) {
        while let Some(child) = self.byline.first_child() {
            self.byline.remove(&child);
        }
        if text.is_empty() {
            return;
        }
        match link {
            Some(link) => self.byline.append(&link_button(text, link)),
            None => self.byline.append(
                &gtk::Label::builder()
                    .label(text)
                    .xalign(0.0)
                    .css_classes(["title-4"])
                    .build(),
            ),
        }
    }

    /// Shows a line of details, for example the year and the song count.
    /// Empty `text` hides the line.
    pub fn set_details(&self, text: &str) {
        self.details.set_label(text);
        self.details.set_visible(!text.is_empty());
    }

    /// Shows a description of plain text. `None` hides it.
    pub fn set_description(&self, text: Option<&str>) {
        self.description.set_text(text);
    }

    /// Adds `button` after the Play and Shuffle buttons.
    pub fn append_button(&self, button: &impl IsA<gtk::Widget>) {
        self.buttons.append(button);
    }

    /// Shows or hides the Shuffle button. An artist page has none.
    pub fn set_shuffle_visible(&self, visible: bool) {
        self.shuffle_button.set_visible(visible);
    }
}

/// The Play and Shuffle actions of a page. The buttons of the header run them.
pub struct PlayActions {
    play: SimpleAction,
    shuffle: SimpleAction,
}

impl PlayActions {
    /// Adds the actions to `page`. They start disabled: the page enables
    /// them when it has songs.
    pub fn install(
        page: &impl IsA<gtk::Widget>,
        on_play: impl Fn() + 'static,
        on_shuffle: impl Fn() + 'static,
    ) -> Self {
        let group = SimpleActionGroup::new();
        let play = SimpleAction::new(PLAY_ACTION, None);
        play.connect_activate(move |_, _| on_play());
        let shuffle = SimpleAction::new(SHUFFLE_ACTION, None);
        shuffle.connect_activate(move |_, _| on_shuffle());
        for action in [&play, &shuffle] {
            action.set_enabled(false);
            group.add_action(action);
        }
        page.insert_action_group(ACTION_GROUP, Some(&group));
        Self { play, shuffle }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.play.set_enabled(enabled);
        self.shuffle.set_enabled(enabled);
    }
}

/// Builds the row with the Play and Shuffle buttons.
fn build_buttons() -> (gtk::Box, gtk::Button) {
    let row = gtk::Box::builder()
        .spacing(12)
        .margin_top(6)
        .halign(gtk::Align::Start)
        .build();
    let play = action_button(
        &gettext("Play"),
        "media-playback-start-symbolic",
        PLAY_ACTION,
    );
    play.add_css_class("suggested-action");
    let shuffle = action_button(
        &gettext("Shuffle"),
        "media-playlist-shuffle-symbolic",
        SHUFFLE_ACTION,
    );
    row.append(&play);
    row.append(&shuffle);
    (row, shuffle)
}

fn action_button(label: &str, icon_name: &str, action: &str) -> gtk::Button {
    gtk::Button::builder()
        .child(
            &adw::ButtonContent::builder()
                .label(label)
                .icon_name(icon_name)
                .build(),
        )
        .action_name(format!("{ACTION_GROUP}.{action}"))
        .css_classes(["pill"])
        .build()
}

fn link_button(text: &str, link: BylineLink<'_>) -> gtk::Button {
    gtk::Button::builder()
        .label(text)
        .halign(gtk::Align::Start)
        .css_classes(["link", "title-4", "byline-link"])
        .action_name(link.action)
        .action_target(&link.target.to_variant())
        .build()
}

/// A description that shows three lines. A long text has a button that shows
/// all of it.
struct Description {
    root: gtk::Box,
    label: gtk::Label,
    toggle: gtk::Button,
}

impl Description {
    fn new() -> Self {
        let label = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(WrapMode::WordChar)
            .lines(COLLAPSED_LINES)
            .ellipsize(EllipsizeMode::End)
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(EXPANDED_MAX_HEIGHT)
            .child(&label)
            .build();
        let toggle = gtk::Button::builder()
            .label(gettext("Show More"))
            .halign(gtk::Align::Start)
            .css_classes(["flat"])
            .build();
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .visible(false)
            .build();
        root.append(&scroller);
        root.append(&toggle);
        let weak_label = label.downgrade();
        toggle.connect_clicked(move |toggle| {
            if let Some(label) = weak_label.upgrade() {
                switch_expanded(&label, toggle);
            }
        });
        Self {
            root,
            label,
            toggle,
        }
    }

    fn widget(&self) -> &gtk::Box {
        &self.root
    }

    fn set_text(&self, text: Option<&str>) {
        self.collapse();
        self.root.set_visible(text.is_some());
        let Some(text) = text else {
            return;
        };
        self.label.set_label(text);
        self.toggle.set_visible(is_long(text));
    }

    fn collapse(&self) {
        if self.label.lines() != COLLAPSED_LINES {
            switch_expanded(&self.label, &self.toggle);
        }
    }
}

/// Shows all lines of `label`, or only the first lines again.
fn switch_expanded(label: &gtk::Label, toggle: &gtk::Button) {
    if label.lines() == COLLAPSED_LINES {
        label.set_lines(-1);
        label.set_ellipsize(EllipsizeMode::None);
        toggle.set_label(&gettext("Show Less"));
    } else {
        label.set_lines(COLLAPSED_LINES);
        label.set_ellipsize(EllipsizeMode::End);
        toggle.set_label(&gettext("Show More"));
    }
}

/// Tells if `text` can need more than the collapsed lines.
fn is_long(text: &str) -> bool {
    text.chars().count() > LONG_DESCRIPTION_CHARS
        || text.lines().count() > usize::try_from(COLLAPSED_LINES).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_long_is_false_for_a_short_text() {
        assert!(!is_long("A short text."));
    }

    #[test]
    fn is_long_is_true_for_many_characters() {
        assert!(is_long(&"a".repeat(LONG_DESCRIPTION_CHARS + 1)));
    }

    #[test]
    fn is_long_is_true_for_many_lines() {
        assert!(is_long("a\nb\nc\nd"));
    }
}
