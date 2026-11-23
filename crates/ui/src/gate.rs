// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Gate in front of the main content. It shows the screen that fits the
//! state of the engine and of the account.

use gettextrs::gettext;
use relm4::adw;
use relm4::gtk;
use relm4::gtk::gio::SimpleAction;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use ringo_core::runner::CoreHandle;
use ringo_core::state::{AppState, AuthState, EngineStatus, Message};

use crate::menu;

/// Page of the Apple Music subscription.
const SUBSCRIBE_URL: &str = "https://music.apple.com/subscribe";

/// Page with the help for the Chromium sandbox errors.
const SANDBOX_HELP_URL: &str =
    "https://github.com/MedaiP90/RingoMusic/blob/main/docs/troubleshooting.md#chromium-sandbox";

/// Page where users report problems.
const ISSUES_URL: &str = "https://github.com/MedaiP90/RingoMusic/issues";

/// Name of the stack child that holds the main content. The parent adds the
/// content to the stack of the gate under this name.
pub const CONTENT_PAGE: &str = "content";

const LOADING_PAGE: &str = "loading";
const FIRST_RUN_PAGE: &str = "first-run";
const NO_SUBSCRIPTION_PAGE: &str = "no-subscription";
const INCOMPATIBLE_PAGE: &str = "incompatible";
const GAVE_UP_PAGE: &str = "gave-up";
const SANDBOX_FAILED_PAGE: &str = "sandbox-failed";

/// What the gate shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Screen {
    /// The engine starts, or the account is not known yet.
    Loading,
    /// The user did not sign in.
    FirstRun,
    /// The account has no Apple Music subscription.
    NoSubscription,
    /// A self-check of the engine failed.
    Incompatible { check: String, detail: String },
    /// The engine failed too often.
    GaveUp,
    /// The Chromium sandbox of the engine cannot start.
    SandboxFailed,
    /// The main content.
    Content,
}

impl Screen {
    /// Chooses the screen for `state`.
    pub fn of(state: &AppState) -> Self {
        match &state.engine {
            EngineStatus::Incompatible { check, detail } => Self::Incompatible {
                check: check.clone(),
                detail: detail.clone(),
            },
            EngineStatus::GaveUp => Self::GaveUp,
            EngineStatus::SandboxFailed => Self::SandboxFailed,
            EngineStatus::Starting => Self::Loading,
            // A restart keeps the screen of the account, so a crash does not hide the content.
            EngineStatus::Ready | EngineStatus::Restarting => Self::of_account(state),
        }
    }

    fn of_account(state: &AppState) -> Self {
        match state.auth {
            AuthState::Unknown => Self::Loading,
            AuthState::SignedOut => Self::FirstRun,
            AuthState::SignedIn { .. } if state.no_subscription => Self::NoSubscription,
            AuthState::SignedIn { .. } => Self::Content,
        }
    }

    /// Name of the stack child of the screen.
    fn page_name(&self) -> &'static str {
        match self {
            Self::Loading => LOADING_PAGE,
            Self::FirstRun => FIRST_RUN_PAGE,
            Self::NoSubscription => NO_SUBSCRIPTION_PAGE,
            Self::Incompatible { .. } => INCOMPATIBLE_PAGE,
            Self::GaveUp => GAVE_UP_PAGE,
            Self::SandboxFailed => SANDBOX_FAILED_PAGE,
            Self::Content => CONTENT_PAGE,
        }
    }

    /// Text of the incompatible page. It is empty for the other screens.
    fn incompatible_description(&self) -> String {
        match self {
            Self::Incompatible { check, detail } => gettext("The check “{check}” failed: {detail}")
                .replace("{check}", check)
                .replace("{detail}", detail),
            _ => String::new(),
        }
    }
}

/// The part of the state that the gate follows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateState {
    screen: Screen,
    signing_in: bool,
}

impl GateState {
    fn of(state: &AppState) -> Self {
        Self {
            screen: Screen::of(state),
            signing_in: state.signing_in,
        }
    }
}

/// Gate model. Its init value is the core handle. The gate also owns the
/// actions of its pages.
pub struct Gate {
    state: GateState,
    sign_in: SimpleAction,
}

#[relm4::component(pub)]
impl SimpleComponent for Gate {
    type Init = CoreHandle;
    type Input = GateState;
    type Output = ();

    view! {
        gtk::Stack {
            set_transition_type: gtk::StackTransitionType::Crossfade,
            add_named[Some(LOADING_PAGE)] = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &menu::primary_button(),
                },

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_title: &gettext("Starting the player…"),

                    #[wrap(Some)]
                    set_child = &adw::Spinner {
                        set_halign: gtk::Align::Center,
                        set_width_request: 32,
                        set_height_request: 32,
                    },
                },
            },

            add_named[Some(FIRST_RUN_PAGE)] = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &menu::primary_button(),
                },

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("audio-headphones-symbolic"),
                    set_title: &gettext("Welcome to RingoMusic"),
                    set_description: Some(&gettext(
                        "RingoMusic plays your Apple Music library. You need an Apple Music subscription. You sign in on a page of Apple, and RingoMusic does not store your password.",
                    )),

                    #[wrap(Some)]
                    set_child = &gtk::Button {
                        set_halign: gtk::Align::Center,
                        set_action_name: Some("app.sign-in"),
                        add_css_class: "suggested-action",
                        add_css_class: "pill",

                        #[wrap(Some)]
                        set_child = &gtk::Box {
                            set_spacing: 8,

                            adw::Spinner {
                                #[watch]
                                set_visible: model.state.signing_in,
                            },

                            gtk::Label {
                                set_label: &gettext("Sign In"),
                            },
                        },
                    },
                },
            },

            add_named[Some(NO_SUBSCRIPTION_PAGE)] = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &menu::primary_button(),
                },

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("dialog-information-symbolic"),
                    set_title: &gettext("No Apple Music subscription"),
                    set_description: Some(&gettext(
                        "This Apple Music account has no subscription. RingoMusic needs a subscription to play music.",
                    )),

                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,

                        gtk::LinkButton {
                            set_uri: SUBSCRIBE_URL,
                            set_label: &gettext("Get Apple Music"),
                        },

                        gtk::Button {
                            set_label: &gettext("Check Again"),
                            set_action_name: Some("app.check-subscription"),
                            add_css_class: "pill",
                        },
                    },
                },
            },

            add_named[Some(INCOMPATIBLE_PAGE)] = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &menu::primary_button(),
                },

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &gettext("The player does not work on this system"),
                    #[watch]
                    set_description: Some(&model.state.screen.incompatible_description()),

                    #[wrap(Some)]
                    set_child = &gtk::LinkButton {
                        set_halign: gtk::Align::Center,
                        set_uri: ISSUES_URL,
                        set_label: &gettext("Report Problem"),
                    },
                },
            },

            add_named[Some(GAVE_UP_PAGE)] = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &menu::primary_button(),
                },

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &gettext("The player stopped"),
                    set_description: Some(&gettext(
                        "The player failed many times and does not start again by itself.",
                    )),

                    #[wrap(Some)]
                    set_child = &gtk::Button {
                        set_halign: gtk::Align::Center,
                        set_label: &gettext("Try Again"),
                        set_action_name: Some("app.restart-engine"),
                        add_css_class: "suggested-action",
                        add_css_class: "pill",
                    },
                },
            },

            add_named[Some(SANDBOX_FAILED_PAGE)] = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &menu::primary_button(),
                },

                #[wrap(Some)]
                set_content = &adw::StatusPage {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_title: &gettext("The player cannot start"),
                    set_description: Some(&gettext(
                        "The security sandbox of the player cannot start. This often means that the installation is not complete, or that the system blocks the sandbox. RingoMusic does not start the player without the sandbox.",
                    )),

                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,

                        gtk::LinkButton {
                            set_uri: SANDBOX_HELP_URL,
                            set_label: &gettext("Open Troubleshooting Guide"),
                        },

                        gtk::Button {
                            set_label: &gettext("Try Again"),
                            set_action_name: Some("app.restart-engine"),
                            add_css_class: "suggested-action",
                            add_css_class: "pill",
                        },
                    },
                },
            },

            // Set after the pages exist. The content page comes later from the parent.
            #[watch]
            set_visible_child_name: model.state.screen.page_name(),
        }
    }

    fn init(
        core: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let state = GateState::of(&core.subscribe().borrow());
        forward_state(&core, &sender);
        let sign_in = register_action("sign-in", &core, Message::SignIn);
        let _ = register_action("restart-engine", &core, Message::RestartEngine);
        let model = Self { state, sign_in };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, state: Self::Input, _sender: ComponentSender<Self>) {
        // The button of the action looks disabled while the login window is open.
        self.sign_in.set_enabled(!state.signing_in);
        self.state = state;
    }
}

/// Adds the application action `name`. The action sends `message` to the core.
fn register_action(name: &str, core: &CoreHandle, message: Message) -> SimpleAction {
    let action = SimpleAction::new(name, None);
    let core = core.clone();
    action.connect_activate(move |_, _| {
        let (core, message) = (core.clone(), message.clone());
        relm4::spawn(async move { core.send(message).await });
    });
    relm4::main_application().add_action(&action);
    action
}

/// Sends the changes of the gate state to the component.
fn forward_state(core: &CoreHandle, sender: &ComponentSender<Gate>) {
    let mut snapshots = core.subscribe();
    let input = sender.input_sender().clone();
    relm4::spawn(async move {
        let mut last = GateState::of(&snapshots.borrow_and_update());
        while snapshots.changed().await.is_ok() {
            let state = GateState::of(&snapshots.borrow_and_update());
            if state != last {
                last = state.clone();
                input.emit(state);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(engine: EngineStatus, auth: AuthState) -> AppState {
        let mut state = AppState::new();
        state.engine = engine;
        state.auth = auth;
        state
    }

    fn signed_in() -> AuthState {
        AuthState::SignedIn {
            storefront: "us".to_owned(),
        }
    }

    #[test]
    fn starting_engine_shows_the_loading_screen() {
        let state = state(EngineStatus::Starting, AuthState::SignedOut);
        assert_eq!(Screen::of(&state), Screen::Loading);
    }

    #[test]
    fn unknown_account_shows_the_loading_screen() {
        let state = state(EngineStatus::Ready, AuthState::Unknown);
        assert_eq!(Screen::of(&state), Screen::Loading);
    }

    #[test]
    fn signed_out_user_sees_the_first_run_screen() {
        let state = state(EngineStatus::Ready, AuthState::SignedOut);
        assert_eq!(Screen::of(&state), Screen::FirstRun);
    }

    #[test]
    fn signed_in_user_sees_the_content() {
        let state = state(EngineStatus::Ready, signed_in());
        assert_eq!(Screen::of(&state), Screen::Content);
    }

    #[test]
    fn restarting_engine_keeps_the_content() {
        let state = state(EngineStatus::Restarting, signed_in());
        assert_eq!(Screen::of(&state), Screen::Content);
    }

    #[test]
    fn missing_subscription_shows_its_screen() {
        let mut state = state(EngineStatus::Ready, signed_in());
        state.no_subscription = true;
        assert_eq!(Screen::of(&state), Screen::NoSubscription);
    }

    #[test]
    fn incompatible_engine_wins_over_the_account() {
        let engine = EngineStatus::Incompatible {
            check: "widevine".to_owned(),
            detail: "no CDM".to_owned(),
        };
        let screen = Screen::of(&state(engine, signed_in()));
        assert_eq!(
            screen,
            Screen::Incompatible {
                check: "widevine".to_owned(),
                detail: "no CDM".to_owned()
            }
        );
    }

    #[test]
    fn engine_with_sandbox_failure_shows_its_screen() {
        let state = state(EngineStatus::SandboxFailed, signed_in());
        assert_eq!(Screen::of(&state), Screen::SandboxFailed);
    }

    #[test]
    fn engine_that_gave_up_shows_its_screen() {
        let state = state(EngineStatus::GaveUp, signed_in());
        assert_eq!(Screen::of(&state), Screen::GaveUp);
    }
}
