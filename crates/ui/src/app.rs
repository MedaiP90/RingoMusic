// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Root component: the main window with the sidebar and the section pages.

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gio::{ActionEntry, SimpleAction};
use relm4::gtk::glib::Propagation;
use relm4::prelude::*;
use ringo_core::artwork::ArtworkStore;
use ringo_core::paths::PackageFormat;
use ringo_core::runner::CoreHandle;
use ringo_core::settings::Settings;
use ringo_core::state::{AppState, AuthState, EngineStatus, NowPlaying};
use ringo_mpris::Callbacks;
use tracing::{debug, warn};

use crate::background::{self, BackgroundPlayback, CloseAction};
use crate::gate::{self, Gate};
use crate::section::Section;
use crate::services::AppServices;
use crate::views::section_page;
use crate::widgets::player_bar::{PlayerBar, PlayerBarInit};
use crate::{about, menu, network_banner, notification, sidebar, subscription};

/// The part of the state that the shell follows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    engine: EngineStatus,
    auth: AuthState,
    sign_in_failed: bool,
}

impl Summary {
    fn of(state: &AppState) -> Self {
        Self {
            engine: state.engine.clone(),
            auth: state.auth.clone(),
            sign_in_failed: state.sign_in_failed,
        }
    }
}

/// What the main window needs to start.
pub struct Init {
    pub services: AppServices,
    pub package: PackageFormat,
}

/// Main window model.
pub struct App {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    section: Section,
    settings: Settings,
    background: BackgroundPlayback,
    sign_in_failed: bool,
}

/// Input of the main window.
#[derive(Debug)]
pub enum Input {
    /// The user picked a section in the sidebar.
    SectionSelected(Section),
    /// The engine or the authorization changed.
    SummaryChanged(Summary),
    /// A D-Bus client asks to show the window.
    Raise,
    /// The user or a D-Bus client asks to quit.
    QuitRequested,
    /// A view asks for a short message.
    ShowToast(String),
    /// The user asks for the about dialog.
    ShowAbout,
    /// The song changed.
    TrackChanged(NowPlaying),
    /// The user closes the window.
    CloseRequested,
    /// The background portal answered.
    PermissionAnswered(bool),
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = Init;
    type Input = Input;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("RingoMusic"),

            connect_close_request[sender] => move |_| {
                sender.input(Input::CloseRequested);
                Propagation::Stop
            },

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &network_banner::build(),

                #[wrap(Some)]
                set_content = &toast_overlay.clone() -> adw::ToastOverlay {
                  #[wrap(Some)]
                  set_child = &gate_stack.clone() -> gtk::Stack {
                    add_named[Some(gate::CONTENT_PAGE)] = &adw::ToolbarView {
                      set_bottom_bar_style: adw::ToolbarStyle::Raised,
                      add_bottom_bar: &player_bar,

                      #[wrap(Some)]
                      set_content = &split_view.clone() -> adw::NavigationSplitView {
                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    set_title: "RingoMusic",

                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        add_top_bar = &adw::HeaderBar {
                            pack_end = &menu::primary_button(),
                        },

                        #[wrap(Some)]
                        set_content = &gtk::ScrolledWindow {
                            set_child: Some(&sidebar_list),
                        },
                    },
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    #[watch]
                    set_title: &model.section.title(),

                    #[wrap(Some)]
                    set_child = &content_stack.clone() -> gtk::Stack {
                        #[watch]
                        set_visible_child_name: model.section.id(),
                    },
                },
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
        let Init { services, package } = init;
        let core = services.core();
        forward_state(core, &sender);
        forward_track_changes(core, &sender);
        publish_mpris(core, services.artwork(), &sender);
        register_actions(&sender);
        subscription::watch(core, services.api());
        let mut gate = Gate::builder().launch(core.clone());
        let gate_stack = gate.widget().clone();
        // The gate lives as long as its widget. A dropped controller would stop its updates.
        gate.detach_runtime();

        let settings = services.settings().clone();
        restore_window_state(&root, &settings);
        let mut bar = PlayerBar::builder().launch(PlayerBarInit {
            core: core.clone(),
            settings: settings.clone(),
            artwork: services.artwork().clone(),
            window: root.clone(),
        });
        let player_bar = bar.widget().clone();
        // Like the gate, the bar lives as long as its widget.
        bar.detach_runtime();
        let background = BackgroundPlayback::new(settings.clone(), package);
        let toast_overlay = adw::ToastOverlay::new();
        if let Some(message) = background.take_refusal_message() {
            sender.input(Input::ShowToast(message));
        }
        let split_view = adw::NavigationSplitView::new();
        let content_stack = build_content_stack(&split_view);
        let sidebar_list = sidebar::build_list();
        connect_sidebar(&sidebar_list, &split_view, &sender);

        let model = Self {
            window: root.clone(),
            toast_overlay: toast_overlay.clone(),
            section: Section::Home,
            settings,
            background,
            sign_in_failed: false,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            Input::SectionSelected(section) => self.section = section,
            Input::SummaryChanged(summary) => self.follow(&summary),
            Input::Raise => self.window.present(),
            Input::QuitRequested => self.quit(),
            Input::ShowToast(message) => self.toast_overlay.add_toast(adw::Toast::new(&message)),
            Input::ShowAbout => about::present(&self.window),
            Input::TrackChanged(track) => self.notify(&track),
            Input::CloseRequested => self.close(&sender),
            Input::PermissionAnswered(granted) => {
                self.background.set_permission(granted);
                if granted {
                    self.hide();
                } else {
                    self.quit();
                }
            }
        }
    }
}

impl App {
    /// Shows a toast when a login fails. One failure gives one toast.
    fn follow(&mut self, summary: &Summary) {
        debug!(?summary, "engine or authorization changed");
        if summary.sign_in_failed && !self.sign_in_failed {
            let message = gettext("Sign in failed. Try again.");
            self.toast_overlay.add_toast(adw::Toast::new(&message));
        }
        self.sign_in_failed = summary.sign_in_failed;
    }

    /// Saves the window state and stops the application. The shutdown handler
    /// then stops the core.
    fn quit(&self) {
        save_window_state(&self.window, &self.settings);
        relm4::main_application().quit();
    }

    /// Quits, hides the window, or asks the portal first.
    fn close(&mut self, sender: &ComponentSender<Self>) {
        save_window_state(&self.window, &self.settings);
        match self.background.close_action() {
            CloseAction::Quit => self.quit(),
            CloseAction::Hide => self.hide(),
            CloseAction::RequestPermission => {
                let input = sender.input_sender().clone();
                relm4::spawn(async move {
                    input.emit(Input::PermissionAnswered(
                        background::request_permission().await,
                    ));
                });
            }
            CloseAction::Wait => {}
        }
    }

    /// Hides the window. The application stays alive and plays on.
    fn hide(&mut self) {
        self.background.keep_alive(&relm4::main_application());
        self.window.set_visible(false);
    }

    /// Shows a notification if the user cannot see the window.
    fn notify(&self, track: &NowPlaying) {
        if self.window.is_active() || !self.settings.show_notifications() {
            return;
        }
        relm4::main_application().send_notification(
            Some(notification::NOTIFICATION_ID),
            &notification::now_playing(track),
        );
    }
}

/// Adds the application actions and their shortcuts. Each action sends an
/// input, so the buttons, the menu, and the keys use the same code.
fn register_actions(sender: &ComponentSender<App>) {
    let (quit_input, about_input) = (sender.input_sender().clone(), sender.input_sender().clone());
    let quit = ActionEntry::builder("quit")
        .activate(move |_: &gtk::Application, _, _| quit_input.emit(Input::QuitRequested))
        .build();
    let about = ActionEntry::builder("about")
        .activate(move |_: &gtk::Application, _, _| about_input.emit(Input::ShowAbout))
        .build();
    // ponytail: prompt 34 adds the preferences dialog. The action is off until then.
    let preferences = SimpleAction::new("preferences", None);
    preferences.set_enabled(false);
    let app = relm4::main_application();
    app.add_action_entries([quit, about]);
    app.add_action(&preferences);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
    app.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
}

/// Builds the stack with the navigation stack of each section.
fn build_content_stack(split_view: &adw::NavigationSplitView) -> gtk::Stack {
    let stack = gtk::Stack::new();
    for section in Section::ALL {
        stack.add_named(
            &section_page::build(section, split_view),
            Some(section.id()),
        );
    }
    stack
}

/// Connects the sidebar list. A selection changes the section. An activation
/// also shows the content, which matters when the split view is collapsed.
fn connect_sidebar(
    list: &gtk::ListBox,
    split_view: &adw::NavigationSplitView,
    sender: &ComponentSender<App>,
) {
    list.select_row(list.row_at_index(0).as_ref());
    let input = sender.input_sender().clone();
    list.connect_row_selected(move |_, row| {
        let section = row.and_then(|row| Section::from_row_index(row.index()));
        if let Some(section) = section {
            input.emit(Input::SectionSelected(section));
        }
    });
    let split_view = split_view.clone();
    list.connect_row_activated(move |_, _| split_view.set_show_content(true));
}

/// Sets the size and the maximized state of `window` from the settings.
fn restore_window_state(window: &adw::ApplicationWindow, settings: &Settings) {
    window.set_default_size(settings.window_width(), settings.window_height());
    if settings.window_maximized() {
        window.maximize();
    }
}

/// Writes the size and the maximized state of `window` to the settings. A
/// maximized window keeps the size from before.
fn save_window_state(window: &adw::ApplicationWindow, settings: &Settings) {
    let maximized = window.is_maximized();
    let mut result = settings.set_window_maximized(maximized);
    if !maximized {
        let (width, height) = window.default_size();
        result = result
            .and_then(|()| settings.set_window_width(width))
            .and_then(|()| settings.set_window_height(height));
    }
    if let Err(error) = result {
        warn!(%error, "cannot save the window state");
    }
}

/// Sends the engine and authorization changes to the window. Position and
/// volume changes do not wake the main thread.
fn forward_state(core: &CoreHandle, sender: &ComponentSender<App>) {
    let mut state = core.subscribe();
    let input = sender.input_sender().clone();
    relm4::spawn(async move {
        let mut last = Summary::of(&state.borrow_and_update());
        while state.changed().await.is_ok() {
            let summary = Summary::of(&state.borrow_and_update());
            if summary != last {
                last = summary.clone();
                input.emit(Input::SummaryChanged(summary));
            }
        }
    });
}

/// Sends each new song to the window. The song at start-up is not new.
fn forward_track_changes(core: &CoreHandle, sender: &ComponentSender<App>) {
    let mut state = core.subscribe();
    let input = sender.input_sender().clone();
    relm4::spawn(async move {
        let mut last_id = state
            .borrow_and_update()
            .now_playing
            .as_ref()
            .map(|track| track.id.clone());
        while state.changed().await.is_ok() {
            let track = state.borrow_and_update().now_playing.clone();
            let id = track.as_ref().map(|track| track.id.clone());
            if id != last_id {
                last_id = id;
                if let Some(track) = track {
                    input.emit(Input::TrackChanged(track));
                }
            }
        }
    });
}

/// Starts the MPRIS server. A failure is not fatal: playback works without it.
fn publish_mpris(core: &CoreHandle, artwork: &ArtworkStore, sender: &ComponentSender<App>) {
    let (raise, quit) = (sender.input_sender().clone(), sender.input_sender().clone());
    let callbacks = Callbacks::new(
        move || raise.emit(Input::Raise),
        move || quit.emit(Input::QuitRequested),
    );
    let (state, inbox, artwork) = (core.subscribe(), core.sender(), artwork.clone());
    relm4::spawn(async move {
        if let Err(error) = ringo_mpris::run(state, inbox, callbacks, artwork).await {
            warn!(%error, "MPRIS is not available");
        }
    });
}
