// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Root component: the main window with the sidebar and the section pages.

use std::sync::Arc;

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
use ringo_core::update_checker::Release;
use ringo_mpris::Callbacks;
use tracing::{debug, warn};

use crate::background::{self, BackgroundPlayback, CloseAction};
use crate::detail_pages::{self, PageSources};
use crate::gate::{self, Gate};
use crate::item_actions::{self, SongCatalog};
use crate::preferences::{Preferences, PreferencesInit};
use crate::section::Section;
use crate::services::AppServices;
use crate::views::library::{LibraryInit, LibraryKind, LibraryView};
use crate::views::queue::{QueueInit, QueueView};
use crate::views::search::{SearchInit, SearchInput, SearchView};
use crate::views::section_page;
use crate::views::sections::{SectionsInit, SectionsInput, SectionsView};
use crate::views::{home, made_for_you, new_releases};
use crate::widgets::player_bar::{PlayerBar, PlayerBarInit, PlayerBarInput, TOGGLE_QUEUE};
use crate::{
    about, accent, artwork, menu, network_banner, notification, sidebar, subscription, theme,
    update_notice,
};

/// Window width in sp below which the sidebar and the queue panel collapse.
const COLLAPSE_WIDTH_SP: f64 = 600.0;

/// Window width in sp below which the player bar shows only the song and
/// play/pause. The full bar needs about 760 px, so it must not apply below this.
const COMPACT_WIDTH_SP: f64 = 800.0;

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
    sidebar_list: gtk::ListBox,
    split_view: adw::NavigationSplitView,
    search: relm4::Sender<SearchInput>,
    home: relm4::Sender<SectionsInput>,
    new_releases: relm4::Sender<SectionsInput>,
    made_for_you: relm4::Sender<SectionsInput>,
    player_bar: relm4::Sender<PlayerBarInput>,
    queue_open: bool,
    services: AppServices,
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
    /// The user asks to reload the current page.
    Refresh,
    /// The user asks to search: show the search page and focus its entry.
    ShowSearch,
    /// The user opens or closes the queue panel.
    ToggleQueue,
    /// The queue panel opened or closed, also by a swipe.
    QueueVisibility(bool),
    /// A view asks for a short message.
    ShowToast(String),
    /// The update check found a newer release.
    UpdateAvailable(Release),
    /// The user asks for the about dialog.
    ShowAbout,
    /// The user asks for the preferences dialog.
    ShowPreferences,
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
                      set_content = &queue_split.clone() -> adw::OverlaySplitView {
                        set_sidebar_position: gtk::PackType::End,
                        set_min_sidebar_width: 320.0,
                        set_max_sidebar_width: 420.0,
                        set_sidebar_width_fraction: 0.3,
                        set_sidebar: Some(&queue_page),
                        #[watch]
                        set_show_sidebar: model.queue_open,
                        connect_show_sidebar_notify[sender] => move |panel| {
                            sender.input(Input::QueueVisibility(panel.shows_sidebar()));
                        },

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
        artwork::install_store(services.artwork().clone());
        accent::install(services.settings());
        theme::install(services.settings());
        forward_state(core, &sender);
        forward_track_changes(core, &sender);
        publish_mpris(core, services.artwork(), &sender);
        register_actions(&sender);
        let catalog = SongCatalog::default();
        register_window_actions(&root, core, &catalog, &sender);
        subscription::watch(core, services.api());
        let mut gate = Gate::builder().launch(core.clone());
        let gate_stack = gate.widget().clone();
        // The gate lives as long as its widget. A dropped controller would stop its updates.
        gate.detach_runtime();

        let settings = services.settings().clone();
        restore_window_state(&root, &settings);
        let compact = narrow_breakpoint(COMPACT_WIDTH_SP);
        let narrow = narrow_breakpoint(COLLAPSE_WIDTH_SP);
        let mut bar = PlayerBar::builder().launch(PlayerBarInit {
            core: core.clone(),
            settings: settings.clone(),
            artwork: services.artwork().clone(),
            window: root.clone(),
            catalog: catalog.clone(),
            api: Arc::clone(services.api()),
            compact: vec![compact.clone(), narrow.clone()],
        });
        let player_bar = bar.widget().clone();
        let bar_input = bar.sender().clone();
        // Like the gate, the bar lives as long as its widget.
        bar.detach_runtime();
        let background = BackgroundPlayback::new(settings.clone(), package);
        let toast_overlay = adw::ToastOverlay::new();
        if let Some(message) = background.take_refusal_message() {
            sender.input(Input::ShowToast(message));
        }
        update_notice::start(&services, &sender);
        let split_view = adw::NavigationSplitView::new();
        let (search_page, search_input) = launch_search(&services, &catalog);
        let [
            (home_page, home_input),
            (new_page, new_input),
            (made_for_you_page, made_for_you_input),
        ] = [
            home::init(services.clone(), catalog.clone()),
            new_releases::init(services.clone(), catalog.clone()),
            made_for_you::init(services.clone(), catalog.clone()),
        ]
        .map(launch_sections);
        register_refresh(&root, sender.input_sender());
        let library_pages = LibraryKind::ALL.map(|kind| launch_library(kind, &services, &catalog));
        let mut queue = QueueView::builder().launch(QueueInit {
            core: core.clone(),
            window: root.clone(),
        });
        let queue_page = queue.widget().clone();
        close_queue_on_escape(&queue_page, &sender);
        // The queue view also lives as long as its widget.
        queue.detach_runtime();
        let content_stack = build_content_stack(
            &home_page,
            &search_page,
            &new_page,
            &made_for_you_page,
            &library_pages,
        );
        detail_pages::register(
            &root,
            PageSources {
                services: services.clone(),
                catalog: catalog.clone(),
                split_view: split_view.clone(),
                sections: content_stack.clone(),
            },
        );
        let queue_split = adw::OverlaySplitView::new();
        let sidebar_list = sidebar::build_list();
        connect_sidebar(&sidebar_list, &split_view, &sender);

        let model = Self {
            window: root.clone(),
            toast_overlay: toast_overlay.clone(),
            section: Section::Home,
            sidebar_list: sidebar_list.clone(),
            split_view: split_view.clone(),
            search: search_input,
            home: home_input,
            new_releases: new_input,
            made_for_you: made_for_you_input,
            player_bar: bar_input,
            queue_open: false,
            services,
            settings,
            background,
            sign_in_failed: false,
        };
        let widgets = view_output!();
        // A window applies one breakpoint at a time, the last one that matches.
        // So the narrower one also gets the setters of the wider one.
        collapse_on(&narrow, &split_view, &queue_split);
        root.add_breakpoint(compact);
        root.add_breakpoint(narrow);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            Input::SectionSelected(section) => self.select(section),
            Input::SummaryChanged(summary) => self.follow(&summary),
            Input::Raise => self.window.present(),
            Input::QuitRequested => self.quit(),
            Input::Refresh => self.refresh(),
            Input::ShowSearch => self.show_search(),
            Input::ToggleQueue => self.set_queue_open(!self.queue_open),
            Input::QueueVisibility(open) => self.set_queue_open(open),
            Input::ShowToast(message) => self.toast_overlay.add_toast(adw::Toast::new(&message)),
            Input::UpdateAvailable(release) => self
                .toast_overlay
                .add_toast(update_notice::toast(&release, &self.window)),
            Input::ShowAbout => about::present(&self.window),
            Input::ShowPreferences => self.show_preferences(),
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

    /// Makes `section` the current one. The refresh action works on the pages
    /// of the shared sections view only.
    fn select(&mut self, section: Section) {
        self.section = section;
        let refresh = self.window.lookup_action("refresh");
        if let Some(refresh) = refresh.and_then(|action| action.downcast::<SimpleAction>().ok()) {
            refresh.set_enabled(matches!(
                section,
                Section::Home | Section::New | Section::MadeForYou
            ));
        }
    }

    /// Reloads the current page, if it can reload.
    fn refresh(&self) {
        match self.section {
            Section::Home => self.home.emit(SectionsInput::Refresh),
            Section::New => self.new_releases.emit(SectionsInput::Refresh),
            Section::MadeForYou => self.made_for_you.emit(SectionsInput::Refresh),
            Section::Search
            | Section::Playlists
            | Section::Albums
            | Section::Artists
            | Section::Songs => {}
        }
    }

    /// Selects the search section and puts the cursor in the search entry.
    fn show_search(&mut self) {
        self.show_section(Section::Search);
        self.search.emit(SearchInput::Focus);
    }

    /// Opens or closes the queue panel and tells the player bar.
    fn set_queue_open(&mut self, open: bool) {
        if self.queue_open != open {
            self.queue_open = open;
            self.player_bar.emit(PlayerBarInput::QueueShown(open));
        }
    }

    /// Selects `section` in the sidebar and shows its page.
    fn show_section(&mut self, section: Section) {
        let row = self.sidebar_list.row_at_index(section.row_index());
        self.sidebar_list.select_row(row.as_ref());
        self.select(section);
        self.split_view.set_show_content(true);
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

    /// Opens the preferences dialog. Like the gate, it lives as long as its widget.
    fn show_preferences(&self) {
        let mut preferences = Preferences::builder().launch(PreferencesInit {
            services: self.services.clone(),
        });
        let dialog = preferences.widget().clone();
        preferences.detach_runtime();
        dialog.present(Some(&self.window));
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
    let preferences_input = sender.input_sender().clone();
    let preferences = ActionEntry::builder("preferences")
        .activate(move |_: &gtk::Application, _, _| preferences_input.emit(Input::ShowPreferences))
        .build();
    let app = relm4::main_application();
    app.add_action_entries([quit, about, preferences]);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
    app.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
}

/// Adds the window actions: the search shortcut and the item actions.
fn register_window_actions(
    window: &adw::ApplicationWindow,
    core: &CoreHandle,
    catalog: &SongCatalog,
    sender: &ComponentSender<App>,
) {
    let input = sender.input_sender().clone();
    let search = ActionEntry::builder("search")
        .activate(move |_: &adw::ApplicationWindow, _, _| input.emit(Input::ShowSearch))
        .build();
    let input = sender.input_sender().clone();
    let toggle_queue = ActionEntry::builder("toggle-queue")
        .activate(move |_: &adw::ApplicationWindow, _, _| input.emit(Input::ToggleQueue))
        .build();
    window.add_action_entries([search, toggle_queue]);
    let application = relm4::main_application();
    application.set_accels_for_action("win.search", &["<Ctrl>f"]);
    application.set_accels_for_action(TOGGLE_QUEUE.action, &[TOGGLE_QUEUE.accelerator]);
    item_actions::register(window, core, catalog);
}

/// Adds the `win.refresh` action. The window sends the refresh to the current page.
fn register_refresh(window: &adw::ApplicationWindow, input: &relm4::Sender<Input>) {
    let input = input.clone();
    let refresh = ActionEntry::builder("refresh")
        .activate(move |_: &adw::ApplicationWindow, _, _| input.emit(Input::Refresh))
        .build();
    window.add_action_entries([refresh]);
    relm4::main_application().set_accels_for_action("win.refresh", &["F5"]);
}

/// Starts the page of a library list. Like the gate, the page lives as long
/// as its widget.
fn launch_library(
    kind: LibraryKind,
    services: &AppServices,
    catalog: &SongCatalog,
) -> (LibraryKind, adw::ToolbarView) {
    let mut view = LibraryView::builder().launch(LibraryInit {
        kind,
        services: services.clone(),
        catalog: catalog.clone(),
    });
    let page = view.widget().clone();
    view.detach_runtime();
    (kind, page)
}

/// Launches the search view. Returns its page and its input sender.
/// The view lives as long as its widget.
fn launch_search(
    services: &AppServices,
    catalog: &SongCatalog,
) -> (adw::ToolbarView, relm4::Sender<SearchInput>) {
    let mut view = SearchView::builder().launch(SearchInit {
        services: services.clone(),
        catalog: catalog.clone(),
    });
    let page = view.widget().clone();
    let input = view.sender().clone();
    view.detach_runtime();
    (page, input)
}

/// Launches a shared sections view. Returns its page and its input sender.
/// The view lives as long as its widget.
fn launch_sections(init: SectionsInit) -> (adw::ToolbarView, relm4::Sender<SectionsInput>) {
    let mut view = SectionsView::builder().launch(init);
    let page = view.widget().clone();
    let input = view.sender().clone();
    view.detach_runtime();
    (page, input)
}

/// Builds the stack with the navigation stack of each section. A section
/// without a view shows an empty page.
fn build_content_stack(
    home_page: &adw::ToolbarView,
    search_page: &adw::ToolbarView,
    new_page: &adw::ToolbarView,
    made_for_you_page: &adw::ToolbarView,
    library_pages: &[(LibraryKind, adw::ToolbarView)],
) -> gtk::Stack {
    let stack = gtk::Stack::new();
    for section in Section::ALL {
        let library_page = library_pages
            .iter()
            .find(|(kind, _)| kind.section() == section);
        let page = match (section, library_page) {
            (_, Some((_, page))) => section_page::wrap(section, page),
            (Section::Home, None) => section_page::wrap(section, home_page),
            (Section::Search, None) => section_page::wrap(section, search_page),
            (Section::New, None) => section_page::wrap(section, new_page),
            (Section::MadeForYou, None) => section_page::wrap(section, made_for_you_page),
            _ => section_page::build(section),
        };
        stack.add_named(&page, Some(section.id()));
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

/// Closes the queue panel when the user presses Escape inside it.
fn close_queue_on_escape(page: &adw::ToolbarView, sender: &ComponentSender<App>) {
    let input = sender.input_sender().clone();
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            input.emit(Input::QueueVisibility(false));
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
    });
    page.add_controller(keys);
}

/// Builds the breakpoint for a window or a bin narrower than `width` sp.
pub(crate) fn narrow_breakpoint(width: f64) -> adw::Breakpoint {
    adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        width,
        adw::LengthUnit::Sp,
    ))
}

/// Collapses the sidebar and the queue panel when `breakpoint` applies.
/// Both then show their page over the content, one at a time.
fn collapse_on(
    breakpoint: &adw::Breakpoint,
    split_view: &adw::NavigationSplitView,
    queue_split: &adw::OverlaySplitView,
) {
    breakpoint.add_setter(split_view, "collapsed", Some(&true.to_value()));
    breakpoint.add_setter(queue_split, "collapsed", Some(&true.to_value()));
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
