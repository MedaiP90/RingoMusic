// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Placeholder main window.
// ponytail: prompt 25 replaces this window with the real shell.

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::glib::Propagation;
use relm4::prelude::*;
use ringo_core::paths::PackageFormat;
use ringo_core::runner::CoreHandle;
use ringo_core::settings::Settings;
use ringo_core::state::{AppState, AuthState, EngineStatus, NowPlaying};
use ringo_mpris::Callbacks;
use tracing::warn;

use crate::background::{self, BackgroundPlayback, CloseAction};
use crate::notification;

/// The part of the state that the placeholder shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    engine: EngineStatus,
    auth: AuthState,
}

impl Summary {
    fn of(state: &AppState) -> Self {
        Self {
            engine: state.engine.clone(),
            auth: state.auth.clone(),
        }
    }
}

/// What the main window needs to start.
pub struct Init {
    pub core: CoreHandle,
    pub settings: Settings,
    pub package: PackageFormat,
}

/// Main window model.
pub struct MainWindow {
    window: adw::ApplicationWindow,
    summary: Summary,
    settings: Settings,
    background: BackgroundPlayback,
}

/// Input of the main window.
#[derive(Debug)]
pub enum Input {
    /// The engine or the authorization changed.
    SummaryChanged(Summary),
    /// A D-Bus client asks to show the window.
    Raise,
    /// A D-Bus client asks to quit.
    Quit,
    /// The song changed.
    TrackChanged(NowPlaying),
    /// The user closes the window.
    CloseRequested,
    /// The background portal answered.
    PermissionAnswered(bool),
}

#[relm4::component(pub)]
impl SimpleComponent for MainWindow {
    type Init = Init;
    type Input = Input;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("RingoMusic"),
            set_default_size: (800, 600),

            connect_close_request[sender] => move |_| {
                sender.input(Input::CloseRequested);
                Propagation::Stop
            },

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &toast_overlay.clone() -> adw::ToastOverlay {
                    #[wrap(Some)]
                    set_child = &adw::StatusPage {
                        set_icon_name: Some("audio-x-generic-symbolic"),
                        set_title: "RingoMusic",
                        #[watch]
                        set_description: Some(&model.description()),
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
        let Init {
            core,
            settings,
            package,
        } = init;
        forward_state(&core, &sender);
        forward_track_changes(&core, &sender);
        publish_mpris(&core, &sender);
        let background = BackgroundPlayback::new(settings.clone(), package);
        let toast_overlay = adw::ToastOverlay::new();
        if let Some(message) = background.take_refusal_message() {
            toast_overlay.add_toast(adw::Toast::new(&message));
        }
        let model = Self {
            window: root.clone(),
            summary: Summary::of(&core.subscribe().borrow()),
            settings,
            background,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            Input::SummaryChanged(summary) => self.summary = summary,
            Input::Raise => self.window.present(),
            Input::Quit => relm4::main_application().quit(),
            Input::TrackChanged(track) => self.notify(&track),
            Input::CloseRequested => self.close(&sender),
            Input::PermissionAnswered(granted) => {
                self.background.set_permission(granted);
                if granted {
                    self.hide();
                } else {
                    relm4::main_application().quit();
                }
            }
        }
    }
}

impl MainWindow {
    /// Quits, hides the window, or asks the portal first.
    fn close(&mut self, sender: &ComponentSender<Self>) {
        match self.background.close_action() {
            CloseAction::Quit => relm4::main_application().quit(),
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

    fn description(&self) -> String {
        format!(
            "Engine: {:?}\nAuthorization: {:?}",
            self.summary.engine, self.summary.auth
        )
    }
}

/// Sends the engine and authorization changes to the window. Position and
/// volume changes do not wake the main thread.
fn forward_state(core: &CoreHandle, sender: &ComponentSender<MainWindow>) {
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
fn forward_track_changes(core: &CoreHandle, sender: &ComponentSender<MainWindow>) {
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
fn publish_mpris(core: &CoreHandle, sender: &ComponentSender<MainWindow>) {
    let (raise, quit) = (sender.input_sender().clone(), sender.input_sender().clone());
    let callbacks = Callbacks::new(
        move || raise.emit(Input::Raise),
        move || quit.emit(Input::Quit),
    );
    let (state, inbox) = (core.subscribe(), core.sender());
    relm4::spawn(async move {
        if let Err(error) = ringo_mpris::run(state, inbox, callbacks).await {
            warn!(%error, "MPRIS is not available");
        }
    });
}
