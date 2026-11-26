// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Header of an album, artist, or playlist page: artwork, title, byline,
//! details, description, and the Play and Shuffle buttons.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::gio::{SimpleAction, SimpleActionGroup};
use relm4::gtk::glib;
use relm4::gtk::pango::{EllipsizeMode, WrapMode};
use relm4::gtk::prelude::*;
use ringo_am_api::write::CatalogItem;

use crate::favorite::FavoriteButton;
use crate::widgets::artwork_image::ArtworkImage;

/// Size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 160;

/// Space between the artwork and the text, in pixels.
const SPACING: i32 = 18;

/// Width that the text needs before it moves under the artwork.
const TEXT_MIN_WIDTH: i32 = 240;

/// Natural width of the title and of the other texts, in characters. A
/// longer text wraps or is cut in the space next to the artwork. Without
/// this limit, the natural width of a long text moves all the texts under
/// the artwork, also in a wide window.
const TITLE_NATURAL_CHARS: i32 = 12;
const TEXT_NATURAL_CHARS: i32 = 30;

/// Size of the description dialog, in pixels.
const DESCRIPTION_DIALOG_WIDTH: i32 = 480;
const DESCRIPTION_DIALOG_HEIGHT: i32 = 480;

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
    favorite: FavoriteButton,
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
            .max_width_chars(TITLE_NATURAL_CHARS)
            .css_classes(["title-1"])
            .build();
        let byline = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let details = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(TEXT_NATURAL_CHARS)
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let description = Description::new(&title);
        let (buttons, shuffle_button) = build_buttons();
        let favorite = FavoriteButton::new();
        buttons.append(favorite.widget());

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
            favorite,
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

    /// Shows the first line of a description of plain text. `None` hides it.
    pub fn set_description(&self, text: Option<&str>) {
        self.description.set_text(text);
    }

    /// Takes the star out of the button row. The page puts it in its header bar.
    pub fn take_favorite_button(&self) -> gtk::Button {
        let button = self.favorite.widget();
        self.buttons.remove(button);
        button.clone()
    }

    /// Shows the star for `item` and reads its status. `None` leaves the star off.
    pub fn show_favorite(&self, item: Option<CatalogItem>) {
        self.favorite.control().show_item(item);
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
        .css_classes(["boxy"])
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

/// A description in one line. When the text does not fit, it is cut and a
/// small Show More button opens a dialog with all of it.
struct Description {
    root: gtk::Overlay,
    label: gtk::Label,
    more: gtk::Button,
}

impl Description {
    /// `title` is the page title. The dialog shows it.
    fn new(title: &gtk::Label) -> Self {
        let label = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .single_line_mode(true)
            .max_width_chars(TEXT_NATURAL_CHARS)
            .ellipsize(EllipsizeMode::End)
            .build();
        let more = gtk::Button::builder()
            .label(gettext("Show More"))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "section-more"])
            .visible(false)
            .build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&label);
        row.append(&more);
        // GTK has no signal for a new label width. An empty drawing area on
        // the row gets the same size and sends `resize`.
        let size_probe = gtk::DrawingArea::builder().can_target(false).build();
        let root = gtk::Overlay::builder().child(&row).visible(false).build();
        root.add_overlay(&size_probe);
        let (weak_label, weak_more) = (label.downgrade(), more.downgrade());
        size_probe.connect_resize(move |_, _, _| {
            if let (Some(label), Some(more)) = (weak_label.upgrade(), weak_more.upgrade()) {
                update_more_later(&label, &more);
            }
        });
        let (weak_label, weak_title) = (label.downgrade(), title.downgrade());
        more.connect_clicked(move |button| {
            if let (Some(label), Some(title)) = (weak_label.upgrade(), weak_title.upgrade()) {
                present_description(button, &title.label(), &label.label());
            }
        });
        Self { root, label, more }
    }

    fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    fn set_text(&self, text: Option<&str>) {
        self.root.set_visible(text.is_some());
        self.label.set_label(text.unwrap_or_default());
        update_more_later(&self.label, &self.more);
    }
}

/// Shows `more` only when `label` is cut. The check waits for the next idle
/// moment: the label has its new width then, and GTK does not let a widget
/// change its visibility while it gets its size.
fn update_more_later(label: &gtk::Label, more: &gtk::Button) {
    let (weak_label, weak_more) = (label.downgrade(), more.downgrade());
    glib::idle_add_local_once(move || {
        if let (Some(label), Some(more)) = (weak_label.upgrade(), weak_more.upgrade()) {
            // The button stays while it is visible and the label is cut. If
            // the label fits next to the button, it also fits without it.
            more.set_visible(label.layout().is_ellipsized());
        }
    });
}

/// Shows a dialog over the window of `parent` with the full `text` and the
/// heading `title`.
fn present_description(parent: &impl IsA<gtk::Widget>, title: &str, text: &str) {
    use relm4::adw::prelude::AdwDialogExt;

    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .valign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(WrapMode::WordChar)
        .selectable(true)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    let toolbar = adw::ToolbarView::builder()
        .content(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_height(true)
                .child(&label)
                .build(),
        )
        .build();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    adw::Dialog::builder()
        .title(title)
        .content_width(DESCRIPTION_DIALOG_WIDTH)
        .content_height(DESCRIPTION_DIALOG_HEIGHT)
        .child(&toolbar)
        .build()
        .present(Some(parent));
}
