// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The states of a detail page: loading, empty, failed, and content. The
//! detail views share the frame. The list views share its loading and failed
//! pages.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::prelude::*;

use crate::views::phase::Phase;

/// Size of the spinner in pixels.
const SPINNER_SIZE: i32 = 32;

/// Pages of a frame, one for each state.
pub struct DetailFrame {
    stack: gtk::Stack,
}

impl DetailFrame {
    /// Makes a frame around `content`. `empty_title` is the title of the empty
    /// state. `on_retry` runs when the user presses "Try Again" after an error.
    pub fn new(
        content: &impl IsA<gtk::Widget>,
        empty_title: &str,
        on_retry: impl Fn() + 'static,
    ) -> Self {
        let stack = gtk::Stack::new();
        stack.add_named(&loading_spinner(), Some(Phase::Loading.page()));
        stack.add_named(
            &adw::StatusPage::builder()
                .icon_name("audio-x-generic-symbolic")
                .title(empty_title)
                .build(),
            Some(Phase::Empty.page()),
        );
        stack.add_named(
            &failed_page(&gettext("Cannot Load This Page"), on_retry),
            Some(Phase::Failed.page()),
        );
        stack.add_named(content, Some(Phase::Content.page()));
        stack.set_visible_child_name(Phase::Loading.page());
        Self { stack }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::Stack {
        &self.stack
    }

    /// Shows the page of `phase`.
    pub fn show(&self, phase: Phase) {
        self.stack.set_visible_child_name(phase.page());
    }
}

/// Makes the spinner of the loading state.
pub fn loading_spinner() -> adw::Spinner {
    adw::Spinner::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .width_request(SPINNER_SIZE)
        .height_request(SPINNER_SIZE)
        .build()
}

/// Makes the page of the failed state. `on_retry` runs when the user presses
/// "Try Again".
pub fn failed_page(title: &str, on_retry: impl Fn() + 'static) -> adw::StatusPage {
    let retry = gtk::Button::builder()
        .label(gettext("Try Again"))
        .halign(gtk::Align::Center)
        .css_classes(["boxy", "suggested-action"])
        .build();
    retry.connect_clicked(move |_| on_retry());
    adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title(title)
        .description(gettext("Check your connection and try again."))
        .child(&retry)
        .build()
}
