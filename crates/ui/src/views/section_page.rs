// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Empty page of a section, until its view exists.
// ponytail: prompts 29-33 replace these pages with the real views.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;

use crate::section::Section;

/// Builds the navigation stack of `section`. Its root page has a header bar
/// with a back button to the sidebar, shown when `split_view` is collapsed.
/// Detail pages push on top of the root page.
pub fn build(section: Section, split_view: &adw::NavigationSplitView) -> adw::NavigationView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.pack_start(&back_button(split_view));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(
        &adw::StatusPage::builder()
            .icon_name(section.icon_name())
            .title(section.title())
            .build(),
    ));
    wrap(section, &toolbar)
}

/// Builds the navigation stack of `section` with `content` as its root page.
/// The content brings its own header bar.
pub fn wrap(section: Section, content: &impl IsA<gtk::Widget>) -> adw::NavigationView {
    let root = adw::NavigationPage::builder()
        .tag(section.id())
        .title(section.title())
        .child(content)
        .build();
    let view = adw::NavigationView::new();
    view.add(&root);
    view
}

/// Builds the button that shows the sidebar again. It is visible only while
/// the split view is collapsed.
pub fn back_button(split_view: &adw::NavigationSplitView) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text(gettext("Back to sidebar"))
        .build();
    button.connect_clicked(glib::clone!(
        #[weak]
        split_view,
        move |_| split_view.set_show_content(false)
    ));
    split_view
        .bind_property("collapsed", &button, "visible")
        .sync_create()
        .build();
    button
}
