// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Window that shows why the application cannot start.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk::glib::markup_escape_text;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

/// Window with one error page. Its init value is the error message.
pub struct StartupError;

#[relm4::component(pub)]
impl SimpleComponent for StartupError {
    type Init = String;
    type Input = ();
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("RingoMusic"),
            set_default_size: (600, 400),

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &gettext("RingoMusic cannot start"),
                    set_description: Some(&markup_escape_text(&message)),
                },
            },
        }
    }

    fn init(
        message: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();
        ComponentParts {
            model: Self,
            widgets,
        }
    }
}
