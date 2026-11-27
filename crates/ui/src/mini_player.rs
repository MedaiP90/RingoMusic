// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The mini player: a small window with the cover and the seek bar below it. It opens when the
//! main window closes and the application plays on.

use std::cell::Cell;
use std::sync::Arc;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gio::Menu;
use relm4::gtk::glib;
use relm4::gtk::pango::EllipsizeMode;
use relm4::prelude::*;
use ringo_core::runner::CoreHandle;

use crate::item_actions::SongCatalog;
use crate::services::AppServices;
use crate::widgets::artwork_image::ArtworkImage;
use crate::widgets::player_bar::{BarLayout, BarState, PlayerBar, PlayerBarInit};

/// Size of the cover in logical pixels. It also sets the width of the window.
const COVER_SIZE: i32 = 320;

/// Space between the menu button and the corner of the cover, and between the song text and the
/// edges of the cover, in logical pixels.
const CORNER_MARGIN: i32 = 12;

/// Space between the song text and the bottom of the cover or the buttons, in logical pixels.
const CAPTION_BAR_SPACING: i32 = 8;

/// Seconds that the hint stays on the cover.
const HINT_SECONDS: u32 = 5;

/// Number of times that the mini player shows the hint in one run. After that, the hint is noise.
const HINT_SHOW_LIMIT: u32 = 2;

/// The application action that shows the main window again.
pub const SHOW_MAIN_WINDOW: &str = "app.show-main-window";

/// The mini player window. It has no window controls. The cover moves the window, and a right
/// click on the cover opens the window menu, where the user can keep the window on top. The seek
/// bar is below the cover. The buttons show over the cover while the pointer is on the window.
pub struct MiniPlayer {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    /// Number of times that the hint showed since the application started.
    hint_count: Cell<u32>,
}

impl MiniPlayer {
    /// Builds the hidden window. Its player bar follows the playback from the start.
    pub fn new(services: &AppServices, catalog: &SongCatalog) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(&relm4::main_application())
            .title("RingoMusic")
            .resizable(false)
            // A libadwaita window is at least 360 px wide. The mini player is as wide as the cover.
            .width_request(COVER_SIZE)
            .hide_on_close(true)
            .build();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&cover_area(services, catalog, &window)));
        window.set_content(Some(&toasts));
        Self {
            window,
            toasts,
            hint_count: Cell::new(0),
        }
    }

    /// Shows the window. The first times after the start, it also shows the hint about the window
    /// menu.
    pub fn show(&self) {
        self.window.present();
        let hint_count = self.hint_count.get();
        if hint_count >= HINT_SHOW_LIMIT {
            return;
        }
        self.hint_count.set(hint_count + 1);
        self.show_hint();
    }

    /// Shows the hint about the window menu on the cover for a short time.
    fn show_hint(&self) {
        // A toast timeout stops while the window is inactive, and the new window often has no
        // focus. So a timer closes the hint.
        let hint = adw::Toast::builder()
            .title(gettext("Right-click to pin"))
            .timeout(0)
            .build();
        self.toasts.add_toast(hint.clone());
        glib::timeout_add_seconds_local_once(HINT_SECONDS, move || hint.dismiss());
    }

    /// Hides the window. It keeps its state for the next show.
    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}

/// Starts the compact player bar. Like the bar of the main window, it lives as long as its widget.
fn launch_bar(
    services: &AppServices,
    catalog: &SongCatalog,
    window: &adw::ApplicationWindow,
    buttons: &gtk::Revealer,
) -> gtk::Box {
    let mut bar = PlayerBar::builder().launch(PlayerBarInit {
        core: services.core().clone(),
        settings: services.settings().clone(),
        artwork: services.artwork().clone(),
        window: window.clone(),
        catalog: catalog.clone(),
        api: Arc::clone(services.api()),
        layout: BarLayout::Mini(buttons.clone()),
    });
    let widget = bar.widget().clone();
    bar.detach_runtime();
    widget
}

/// Builds the cover with the song text over it and the seek bar below it. The window moves with
/// a press that no button takes. The menu button and the bar buttons show on hover.
fn cover_area(
    services: &AppServices,
    catalog: &SongCatalog,
    window: &adw::ApplicationWindow,
) -> gtk::WindowHandle {
    let song = SongView::new(services);
    let caption = song.caption.clone();
    let menu_button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(gettext("Menu"))
        .menu_model(&menu())
        .css_classes(["cover-menu", "circular"])
        .build();
    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::Crossfade)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Start)
        .margin_top(CORNER_MARGIN)
        .margin_end(CORNER_MARGIN)
        .child(&menu_button)
        .build();
    let buttons = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .build();
    let bar = launch_bar(services, catalog, window, &buttons);
    // The song text sits on the buttons, so it moves up when they show.
    let bottom = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::End)
        .build();
    bottom.append(&caption);
    bottom.append(&buttons);
    // The overlay takes its size from the cover only, so the buttons do not change the window.
    let cover = gtk::Overlay::builder().child(song.cover.widget()).build();
    follow_song(services.core(), song);
    cover.add_overlay(&revealer);
    cover.add_overlay(&bottom);
    let area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    area.append(&cover);
    area.append(&bar);
    reveal_on_hover(&area, &[revealer, buttons], &menu_button);
    gtk::WindowHandle::builder().child(&area).build()
}

/// The items of the cover menu.
fn menu() -> Menu {
    let menu = Menu::new();
    menu.append(Some(&gettext("_Open RingoMusic")), Some(SHOW_MAIN_WINDOW));
    menu.append(Some(&gettext("_Quit")), Some("app.quit"));
    menu
}

/// The cover of the current song, and its title and artist.
struct SongView {
    cover: ArtworkImage,
    /// The box of the song text. A backdrop with the cover tint keeps it readable.
    caption: gtk::Box,
    title: gtk::Label,
    artist: gtk::Label,
}

impl SongView {
    fn new(services: &AppServices) -> Self {
        let cover = ArtworkImage::new(services.artwork().clone());
        // The cover fills the top of the window, edge to edge, so it has no card look.
        cover.widget().remove_css_class("card");
        let (title, artist) = (caption_label("heading"), caption_label("caption"));
        // The text lets clicks through, so the whole cover still moves the window.
        let caption = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .margin_start(CORNER_MARGIN)
            .margin_end(CORNER_MARGIN)
            .margin_bottom(CAPTION_BAR_SPACING)
            .can_target(false)
            .css_classes(["cover-caption"])
            .build();
        caption.append(&title);
        caption.append(&artist);
        Self {
            cover,
            caption,
            title,
            artist,
        }
    }

    /// Shows `song`. Without a song, the text is hidden.
    fn show(&self, song: &BarState) {
        // The same template again does nothing.
        self.cover.set_artwork(song.artwork(), COVER_SIZE);
        self.title.set_label(song.title());
        self.artist.set_label(song.artist());
        self.caption.set_visible(!song.title().is_empty());
    }
}

/// A line of the song text. A long text ends with an ellipsis.
fn caption_label(css_class: &str) -> gtk::Label {
    gtk::Label::builder()
        .justify(gtk::Justification::Center)
        .ellipsize(EllipsizeMode::End)
        .css_classes([css_class])
        .build()
}

/// Shows the current song in `view`.
fn follow_song(core: &CoreHandle, view: SongView) {
    let mut snapshots = core.subscribe();
    // The widgets are not `Send`, so the task runs on the main thread.
    glib::spawn_future_local(async move {
        loop {
            view.show(&BarState::of(&snapshots.borrow_and_update()));
            if snapshots.changed().await.is_err() {
                break;
            }
        }
    });
}

/// Shows `revealers` while the pointer is on `area` or while the menu of `button` is open.
fn reveal_on_hover(area: &gtk::Box, revealers: &[gtk::Revealer], button: &gtk::MenuButton) {
    let reveal = {
        let revealers: Vec<glib::WeakRef<gtk::Revealer>> =
            revealers.iter().map(ObjectExt::downgrade).collect();
        move |shown: bool| {
            for revealer in revealers.iter().filter_map(glib::WeakRef::upgrade) {
                revealer.set_reveal_child(shown);
            }
        }
    };
    let motion = gtk::EventControllerMotion::new();
    let on_enter = reveal.clone();
    motion.connect_enter(move |_, _, _| on_enter(true));
    let on_leave = reveal.clone();
    motion.connect_leave(glib::clone!(
        #[weak]
        button,
        move |_| on_leave(button.is_active())
    ));
    button.connect_active_notify(glib::clone!(
        #[weak]
        motion,
        move |button| reveal(button.is_active() || motion.contains_pointer())
    ));
    area.add_controller(motion);
}
