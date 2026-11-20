// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Placeholder main window.
// ponytail: prompt 25 replaces this window with the real shell.

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::prelude::*;
use ringo_core::runner::CoreHandle;
use ringo_core::state::{AppState, AuthState, EngineStatus};
use ringo_mpris::Callbacks;
use tracing::warn;

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

/// Main window model.
pub struct MainWindow {
    window: adw::ApplicationWindow,
    summary: Summary,
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
}

#[relm4::component(pub)]
impl SimpleComponent for MainWindow {
    type Init = CoreHandle;
    type Input = Input;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("RingoMusic"),
            set_default_size: (800, 600),

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("audio-x-generic-symbolic"),
                    set_title: "RingoMusic",
                    #[watch]
                    set_description: Some(&model.description()),
                },
            },
        }
    }

    fn init(
        core: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        forward_state(&core, &sender);
        publish_mpris(&core, &sender);
        let model = Self {
            window: root.clone(),
            summary: Summary::of(&core.subscribe().borrow()),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            Input::SummaryChanged(summary) => self.summary = summary,
            Input::Raise => self.window.present(),
            Input::Quit => relm4::main_application().quit(),
        }
    }
}

impl MainWindow {
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
