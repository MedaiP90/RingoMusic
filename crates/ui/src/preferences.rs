// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The preferences dialog: user settings and account actions.

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::glib;
use relm4::prelude::*;
use ringo_core::settings::Settings;
use ringo_core::state::{AuthState, Message};
use ringo_core::update_checker;

use crate::services::AppServices;
use crate::{accent, disclaimer, theme};

/// Response ID of the confirmation dialog for the sign-out button.
const SIGN_OUT_RESPONSE: &str = "sign-out";

/// Step and page size of the cache limit row, in mebibytes.
const CACHE_STEP_MB: f64 = 16.0;

/// Bytes in one mebibyte.
const MEBIBYTE: u64 = 1024 * 1024;

/// What the dialog needs to start.
pub struct PreferencesInit {
    pub services: AppServices,
}

/// Model of the preferences dialog.
pub struct Preferences {
    services: AppServices,
    dialog: adw::PreferencesDialog,
    account_title: String,
    account_subtitle: String,
    signed_in: bool,
    clearing: bool,
    /// Size of the cached API responses, in bytes.
    api_bytes: u64,
    /// Size of the artwork files, in bytes.
    artwork_bytes: u64,
    /// Limit of the API cache, in mebibytes.
    api_limit_mb: u32,
    /// Limit of the artwork cache, in mebibytes.
    artwork_limit_mb: u32,
}

/// Input of the preferences dialog.
#[derive(Debug)]
pub enum Input {
    /// The user presses "Sign Out". The dialog asks for confirmation.
    ConfirmSignOut,
    /// The user confirmed the sign-out.
    SignOut,
    /// The user presses "Clear Cache".
    ClearCache,
    /// The user picked a theme. The value is its position in the list.
    ThemeSelected(u32),
    /// The user picked an accent color. The value is its position in the list.
    AccentSelected(u32),
    /// The user changed the API cache size limit. The value is in mebibytes.
    ApiLimitChanged(u32),
    /// The user changed the artwork cache size limit. The value is in mebibytes.
    ArtworkLimitChanged(u32),
    /// The sizes of the caches. They are loaded at start.
    UsageLoaded { api_bytes: u64, artwork_bytes: u64 },
    /// The caches are empty. The value is the number of freed bytes.
    CacheCleared(u64),
}

#[relm4::component(pub)]
impl SimpleComponent for Preferences {
    type Init = PreferencesInit;
    type Input = Input;
    type Output = ();

    view! {
        #[root]
        adw::PreferencesDialog {
            add = &adw::PreferencesPage {
                set_title: &gettext("General"),
                set_icon_name: Some("preferences-system-symbolic"),

                add = &adw::PreferencesGroup {
                    #[name = "quit_row"]
                    add = &adw::SwitchRow {
                        set_title: &gettext("Quit when the window is closed"),
                    },
                    #[name = "mini_player_row"]
                    add = &adw::SwitchRow {
                        set_title: &gettext("Show a mini player when the window is closed"),
                    },
                    #[name = "notifications_row"]
                    add = &adw::SwitchRow {
                        set_title: &gettext("Show notifications for new songs"),
                    },
                    #[name = "updates_row"]
                    add = &adw::SwitchRow {
                        set_title: &gettext("Check for updates"),
                        set_subtitle: &gettext("Look for a new version when the application starts"),
                        // The package updates the application when the marker file exists.
                        set_visible: !update_checker::disabled_by_package(
                            &model.services.paths().install_prefix,
                        ),
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: &gettext("Appearance"),
                    add = &adw::ComboRow {
                        set_title: &gettext("Theme"),
                        set_model: Some(&labels(&theme::IDS, theme::label)),
                        set_selected: position_in(&theme::IDS, &model.services.settings().color_scheme()),
                        connect_selected_notify[sender] => move |row| {
                            sender.input(Input::ThemeSelected(row.selected()));
                        },
                    },
                    add = &adw::ComboRow {
                        set_title: &gettext("Accent color"),
                        set_model: Some(&labels(&accent::IDS, accent::label)),
                        set_selected: position_in(&accent::IDS, &model.services.settings().accent_color()),
                        connect_selected_notify[sender] => move |row| {
                            sender.input(Input::AccentSelected(row.selected()));
                        },
                    },
                },
            },

            add = &adw::PreferencesPage {
                set_title: &gettext("Storage"),
                set_icon_name: Some("drive-harddisk-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &gettext("API cache"),
                    #[name = "api_limit_row"]
                    add = &adw::SpinRow {
                        set_title: &gettext("Size limit"),
                        set_subtitle: &gettext("Size in MiB. A new limit applies after a restart."),
                        set_adjustment: Some(&limit_adjustment(50.0)),
                        connect_value_notify[sender] => move |row| {
                            sender.input(Input::ApiLimitChanged(spin_value_mb(row)));
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &gettext("In use"),
                        #[watch]
                        set_subtitle: &model.api_text(),
                        add_suffix = &gtk::ProgressBar {
                            set_valign: gtk::Align::Center,
                            set_width_request: 140,
                            #[watch]
                            set_fraction: model.api_fraction(),
                        },
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: &gettext("Artwork cache"),
                    #[name = "artwork_limit_row"]
                    add = &adw::SpinRow {
                        set_title: &gettext("Size limit"),
                        set_subtitle: &gettext("Size in MiB. A new limit applies after a restart."),
                        set_adjustment: Some(&limit_adjustment(500.0)),
                        connect_value_notify[sender] => move |row| {
                            sender.input(Input::ArtworkLimitChanged(spin_value_mb(row)));
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &gettext("In use"),
                        #[watch]
                        set_subtitle: &model.artwork_text(),
                        add_suffix = &gtk::ProgressBar {
                            set_valign: gtk::Align::Center,
                            set_width_request: 140,
                            #[watch]
                            set_fraction: model.artwork_fraction(),
                        },
                    },
                },

                add = &adw::PreferencesGroup {
                    add = &adw::ButtonRow {
                        set_title: &gettext("Clear Cache"),
                        #[watch]
                        set_sensitive: !model.clearing,
                        connect_activated => Input::ClearCache,
                    },
                },
            },

            add = &adw::PreferencesPage {
                set_title: &gettext("Account"),
                set_icon_name: Some("avatar-default-symbolic"),

                add = &adw::PreferencesGroup {
                    add = &adw::ActionRow {
                        set_title: &model.account_title,
                        set_subtitle: &model.account_subtitle,
                    },
                    add = &adw::ButtonRow {
                        set_title: &gettext("Sign Out"),
                        add_css_class: "destructive-action",
                        set_sensitive: model.signed_in,
                        connect_activated => Input::ConfirmSignOut,
                    },
                },

                add = &adw::PreferencesGroup {
                    // Collapsed by default, so the long text does not make the dialog taller.
                    add = &adw::ExpanderRow {
                        set_title: &gettext("Library Functions"),
                        add_row = &gtk::Label {
                            set_label: &disclaimer::body(),
                            set_wrap: true,
                            set_xalign: 0.0,
                            set_selectable: true,
                            set_margin_all: 12,
                        },
                        add_row = &adw::ButtonRow {
                            set_title: &gettext("Open music.apple.com"),
                            connect_activated[dialog = root.clone()] => move |_| {
                                disclaimer::open_music_site(&dialog);
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let services = init.services;
        let (account_title, account_subtitle, signed_in) =
            describe_account(&services.core().subscribe().borrow().auth);
        let api_limit_mb = services.settings().api_cache_max_mb();
        let artwork_limit_mb = services.settings().artwork_cache_max_mb();
        let model = Self {
            services,
            dialog: root.clone(),
            account_title,
            account_subtitle,
            signed_in,
            clearing: false,
            api_bytes: 0,
            artwork_bytes: 0,
            api_limit_mb,
            artwork_limit_mb,
        };
        let widgets = view_output!();
        bind_settings(&model.services, &widgets);
        model.load_usage(&sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            Input::ConfirmSignOut => confirm_sign_out(&self.dialog, &sender),
            Input::SignOut => self.sign_out(),
            Input::ClearCache => self.clear_cache(&sender),
            Input::ThemeSelected(position) => {
                self.save_choice(&theme::IDS, position, Settings::set_color_scheme);
            }
            Input::AccentSelected(position) => {
                self.save_choice(&accent::IDS, position, Settings::set_accent_color);
            }
            Input::ApiLimitChanged(limit_mb) => self.api_limit_mb = limit_mb,
            Input::ArtworkLimitChanged(limit_mb) => self.artwork_limit_mb = limit_mb,
            Input::UsageLoaded {
                api_bytes,
                artwork_bytes,
            } => {
                self.api_bytes = api_bytes;
                self.artwork_bytes = artwork_bytes;
            }
            Input::CacheCleared(bytes) => {
                self.clearing = false;
                self.api_bytes = 0;
                self.artwork_bytes = 0;
                let size = glib::format_size(bytes);
                let message = gettext("Cache cleared. {size} freed.").replace("{size}", &size);
                self.dialog.add_toast(adw::Toast::new(&message));
            }
        }
    }
}

impl Preferences {
    /// Saves the choice at `position` of `ids` with `save`. The application shows it at once.
    fn save_choice(
        &self,
        ids: &[&str],
        position: u32,
        save: fn(&Settings, &str) -> ringo_core::Result<()>,
    ) {
        let id = usize::try_from(position)
            .ok()
            .and_then(|index| ids.get(index));
        let Some(id) = id else {
            return;
        };
        if let Err(error) = save(self.services.settings(), id) {
            tracing::warn!(%error, "cannot save the choice");
        }
    }

    /// Reads the sizes of the caches off the main thread.
    fn load_usage(&self, sender: &ComponentSender<Self>) {
        let api = self.services.api().clone();
        let artwork = self.services.artwork().clone();
        let input = sender.input_sender().clone();
        relm4::spawn(async move {
            let api_bytes = api.used_bytes().await.unwrap_or_else(|error| {
                tracing::warn!(%error, "cannot read the size of the API cache");
                0
            });
            let artwork_bytes = tokio::task::spawn_blocking(move || artwork.used_bytes())
                .await
                .unwrap_or_default();
            input.emit(Input::UsageLoaded {
                api_bytes,
                artwork_bytes,
            });
        });
    }

    /// Share of the API cache limit that the responses use, from 0.0 to 1.0.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the cache size is far below 2^52"
    )]
    fn api_fraction(&self) -> f64 {
        let limit = u64::from(self.api_limit_mb) * MEBIBYTE;
        (self.api_bytes as f64 / limit as f64).min(1.0)
    }

    /// Share of the artwork limit that the files use, from 0.0 to 1.0.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the cache size is far below 2^52"
    )]
    fn artwork_fraction(&self) -> f64 {
        let limit = u64::from(self.artwork_limit_mb) * MEBIBYTE;
        (self.artwork_bytes as f64 / limit as f64).min(1.0)
    }

    /// Text under the API bar: the used size against the limit.
    fn api_text(&self) -> String {
        used_of_limit(self.api_bytes, u64::from(self.api_limit_mb) * MEBIBYTE)
    }

    /// Text under the artwork bar: the used size against the fixed limit.
    fn artwork_text(&self) -> String {
        used_of_limit(
            self.artwork_bytes,
            u64::from(self.artwork_limit_mb) * MEBIBYTE,
        )
    }

    /// Clears the queue, signs out, and closes the dialog. The gate then shows
    /// the first-run page.
    fn sign_out(&self) {
        let core = self.services.core().clone();
        relm4::spawn(async move {
            core.send(Message::ClearQueue).await;
            core.send(Message::SignOut).await;
        });
        self.dialog.close();
    }

    /// Deletes the API cache rows and the artwork files off the main thread.
    fn clear_cache(&mut self, sender: &ComponentSender<Self>) {
        self.clearing = true;
        let api = self.services.api().clone();
        let artwork = self.services.artwork().clone();
        let input = sender.input_sender().clone();
        relm4::spawn(async move {
            let rows = api.clear().await.unwrap_or_else(|error| {
                tracing::warn!(%error, "cannot clear the API cache");
                0
            });
            let files = tokio::task::spawn_blocking(move || artwork.clear())
                .await
                .unwrap_or_default();
            input.emit(Input::CacheCleared(rows + files));
        });
    }
}

/// Returns the title and the subtitle of the account row, and whether the
/// user is signed in.
fn describe_account(auth: &AuthState) -> (String, String, bool) {
    match auth {
        AuthState::SignedIn { storefront } => (
            gettext("Signed in"),
            gettext("Storefront: {storefront}").replace("{storefront}", storefront),
            true,
        ),
        AuthState::SignedOut | AuthState::Unknown => {
            (gettext("Not signed in"), String::new(), false)
        }
    }
}

/// Binds the rows to the settings. `GSettings` writes each change at once.
fn bind_settings(services: &AppServices, widgets: &PreferencesWidgets) {
    let settings = services.settings().raw();
    settings
        .bind("quit-on-close", &widgets.quit_row, "active")
        .build();
    settings
        .bind("mini-player", &widgets.mini_player_row, "active")
        .build();
    // The mini player needs the application to stay alive after the close.
    settings
        .bind("quit-on-close", &widgets.mini_player_row, "sensitive")
        .get()
        .invert_boolean()
        .build();
    settings
        .bind("show-notifications", &widgets.notifications_row, "active")
        .build();
    settings
        .bind("check-updates", &widgets.updates_row, "active")
        .build();
    settings
        .bind("api-cache-max-mb", &widgets.api_limit_row, "value")
        .build();
    settings
        .bind("artwork-cache-max-mb", &widgets.artwork_limit_row, "value")
        .build();
}

/// Asks the user to confirm the sign-out.
fn confirm_sign_out(parent: &adw::PreferencesDialog, sender: &ComponentSender<Preferences>) {
    let alert = adw::AlertDialog::new(
        Some(&gettext("Sign out?")),
        Some(&gettext("Playback stops and the queue is cleared.")),
    );
    alert.add_response("cancel", &gettext("_Cancel"));
    alert.add_response(SIGN_OUT_RESPONSE, &gettext("_Sign Out"));
    alert.set_response_appearance(SIGN_OUT_RESPONSE, adw::ResponseAppearance::Destructive);
    alert.set_default_response(Some("cancel"));
    alert.set_close_response("cancel");
    let input = sender.input_sender().clone();
    alert.connect_response(Some(SIGN_OUT_RESPONSE), move |_, _| {
        input.emit(Input::SignOut);
    });
    alert.present(Some(parent));
}

/// Returns a list with the user names of `ids`, as `name` gives them.
fn labels(ids: &[&str], name: fn(&str) -> String) -> gtk::StringList {
    let list = gtk::StringList::default();
    for id in ids {
        list.append(&name(id));
    }
    list
}

/// Returns the position of `id` in `ids`. An unknown value gives the first entry, the
/// system entry.
fn position_in(ids: &[&str], id: &str) -> u32 {
    ids.iter()
        .position(|candidate| *candidate == id)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// Makes the adjustment of a cache limit row, from 16 to 4096 MiB.
fn limit_adjustment(default_mb: f64) -> gtk::Adjustment {
    gtk::Adjustment::new(
        default_mb,
        16.0,
        4096.0,
        CACHE_STEP_MB,
        CACHE_STEP_MB * 4.0,
        0.0,
    )
}

/// Formats "`used` of `limit`" with readable sizes.
fn used_of_limit(used: u64, limit: u64) -> String {
    gettext("{used} of {limit}")
        .replace("{used}", &glib::format_size(used))
        .replace("{limit}", &glib::format_size(limit))
}

/// Returns the value of the limit row in mebibytes. The row range is 16 to 4096.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the adjustment keeps the value between 16 and 4096"
)]
fn spin_value_mb(row: &adw::SpinRow) -> u32 {
    row.value().round() as u32
}
