// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Player bar: the current song, the transport buttons, the seek bar, the
//! volume, and the shuffle and repeat buttons.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gdk::EventType;
use relm4::gtk::glib::{self, Propagation};
use relm4::gtk::pango::EllipsizeMode;
use relm4::prelude::*;
use ringo_core::artwork::ArtworkStore;
use ringo_core::runner::CoreHandle;
use ringo_core::settings::Settings;
use ringo_core::state::{AppState, Message};
use ringo_engine::protocol::{PlaybackState, RepeatMode};
use tracing::warn;

use crate::add_to_playlist::ItemMenu;
use crate::detail;
use crate::item_actions::{OPEN_ALBUM, OPEN_ARTIST, SongCatalog, SongLinks};
use crate::playback_actions::{self as shortcuts, ActionState, PlaybackActions};
use crate::services::ApiClient;
use crate::tint;
use crate::widgets::artwork_image::ArtworkImage;

/// Size of the artwork in pixels.
const ARTWORK_SIZE: i32 = 64;

/// The part of the state that the bar shows. The position has whole seconds,
/// so the bar wakes up at most once for each second.
#[derive(Clone, Debug, PartialEq)]
pub struct BarState {
    title: String,
    artist: String,
    /// ID of the current song.
    song_id: Option<String>,
    /// Album of the current song, if the engine or a view knows it.
    album_id: Option<String>,
    /// Artist of the current song, if a view knows it.
    artist_id: Option<String>,
    /// Template of the artwork URL.
    artwork: Option<String>,
    duration: Duration,
    position: Duration,
    playing: bool,
    volume: f64,
    shuffle: bool,
    repeat: RepeatMode,
    has_queue: bool,
}

impl BarState {
    fn of(state: &AppState) -> Self {
        // Before the engine plays, the queue knows the current song.
        let (title, artist, artwork, duration) = match (&state.now_playing, state.queue.current()) {
            (Some(track), _) => (
                &track.title,
                &track.artist,
                &track.artwork_url,
                track.duration,
            ),
            (None, Some(item)) => (
                &item.title,
                &item.artist,
                &item.artwork_template,
                item.duration,
            ),
            (None, None) => (&String::new(), &String::new(), &None, Duration::ZERO),
        };
        Self {
            title: title.clone(),
            artist: artist.clone(),
            song_id: state
                .now_playing
                .as_ref()
                .map(|track| track.id.to_string())
                .or_else(|| state.queue.current().map(|item| item.id.to_string())),
            album_id: state
                .now_playing
                .as_ref()
                .and_then(|track| track.album_id.clone()),
            artist_id: None,
            artwork: artwork.clone(),
            duration,
            position: Duration::from_secs(state.playback.position.as_secs()),
            playing: state.playback.state == PlaybackState::Playing,
            volume: f64::from(state.playback.volume),
            shuffle: state.queue.shuffle_enabled(),
            repeat: state.queue.repeat(),
            has_queue: !state.queue.items().is_empty(),
        }
    }

    /// Takes the album and the artist from the catalog. The engine sends only
    /// the album, and not always, so its album is the last choice.
    fn fill_links(&mut self, catalog: &SongCatalog) {
        let Some(song_id) = self.song_id.as_deref() else {
            return;
        };
        self.album_id = catalog.album_id(song_id).or_else(|| self.album_id.take());
        self.artist_id = catalog.artist_id(song_id);
    }

    /// Tells if the album or the artist of the song is still unknown.
    fn lacks_links(&self) -> bool {
        self.song_id.is_some() && (self.album_id.is_none() || self.artist_id.is_none())
    }

    fn action_state(&self) -> ActionState {
        ActionState {
            has_queue: self.has_queue,
            shuffle: self.shuffle,
        }
    }
}

/// What the player bar needs to start.
pub struct PlayerBarInit {
    pub core: CoreHandle,
    pub settings: Settings,
    /// The artwork files.
    pub artwork: ArtworkStore,
    /// The window that gets the playback actions.
    pub window: adw::ApplicationWindow,
    /// The songs that the views showed. Their albums fill the gaps of the engine.
    pub catalog: SongCatalog,
    /// Finds the album and the artist of a song that no view showed.
    pub api: Arc<ApiClient>,
    /// Breakpoints for narrow windows. The bar adds its own setters. The
    /// window owner adds the breakpoints to the window.
    pub compact: Vec<adw::Breakpoint>,
}

/// Player bar model.
pub struct PlayerBar {
    state: BarState,
    actions: PlaybackActions,
    settings: Settings,
    core: CoreHandle,
    catalog: SongCatalog,
    api: Arc<ApiClient>,
    window: adw::ApplicationWindow,
    /// IDs of the songs whose links were asked for. Each song is asked once.
    asked_links: HashSet<String>,
    artwork: ArtworkImage,
    /// The menu that adds the current song to a playlist or to the library.
    add_menu: ItemMenu,
    /// The user holds the seek bar. Position updates wait until the release.
    dragging: bool,
    /// The last position that the user chose while the seek bar was held.
    seek_target: Option<Duration>,
    /// The queue panel is open.
    queue_open: bool,
}

/// Input of the player bar.
#[derive(Debug)]
pub enum PlayerBarInput {
    /// The state changed.
    State(BarState),
    /// The user pressed the seek bar.
    SeekPressed,
    /// The user moved the seek bar to this many seconds.
    SeekMoved(f64),
    /// The user released the seek bar.
    SeekReleased,
    /// The user moved the volume bar to this value.
    VolumeMoved(f64),
    /// The volume popover closed. The bar shows the volume again.
    VolumePopoverClosed,
    /// The queue panel opened or closed.
    QueueShown(bool),
    /// The user pressed the cover.
    OpenAlbum,
    /// The user pressed the artist.
    OpenArtist,
    /// The API gave the album and the artist of a song.
    LinksLoaded(String, SongLinks),
}

#[relm4::component(pub)]
impl SimpleComponent for PlayerBar {
    type Init = PlayerBarInit;
    type Input = PlayerBarInput;
    type Output = ();

    view! {
        gtk::Box {
            set_spacing: 16,
            add_css_class: "player-bar",
            add_css_class: "cover-tint",
            #[watch]
            set_visible: model.state.has_queue,

            // The cover and the song text. They stay on one row.
            #[name = "song_box"]
            gtk::Box {
                set_spacing: 16,

            // The cover opens the album of the song. Without an album ID, it does
            // nothing and the pointer stays an arrow. It is never greyed out.
            gtk::Button {
                add_css_class: "flat",
                add_css_class: "artwork-button",
                set_valign: gtk::Align::Center,
                set_sensitive: true,
                set_tooltip_text: Some(&gettext("Open album")),
                #[watch]
                set_can_target: model.state.album_id.is_some(),
                #[watch]
                set_cursor_from_name: model.state.album_id.as_ref().map(|_| "pointer"),
                connect_clicked[sender] => move |_| sender.input(PlayerBarInput::OpenAlbum),

                #[local_ref]
                #[wrap(Some)]
                set_child = &artwork_widget -> gtk::Image {},
            },

            #[name = "info_box"]
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_valign: gtk::Align::Center,
                set_width_request: 220,

                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_xalign: 0.0,
                    set_ellipsize: EllipsizeMode::End,
                    set_max_width_chars: 24,
                    add_css_class: "heading",
                    #[watch]
                    set_label: &model.state.title,
                    #[watch]
                    set_tooltip_text: Some(&model.state.title),
                },

                // The artist opens the artist page when a view knows its ID.
                gtk::Button {
                    set_halign: gtk::Align::Start,
                    add_css_class: "flat",
                    add_css_class: "title-button",
                    set_sensitive: true,
                    #[watch]
                    set_can_target: model.state.artist_id.is_some(),
                    #[watch]
                    set_cursor_from_name: model.state.artist_id.as_ref().map(|_| "pointer"),
                    connect_clicked[sender] => move |_| sender.input(PlayerBarInput::OpenArtist),

                    #[wrap(Some)]
                    set_child = &gtk::Label {
                        set_xalign: 0.0,
                        set_ellipsize: EllipsizeMode::End,
                        set_max_width_chars: 24,
                        add_css_class: "dim-label",
                        #[watch]
                        set_label: &model.state.artist,
                    },
                },
            },

            },

            // The groups of controls. A narrow window stacks them.
            #[name = "controls"]
            gtk::Box {
                set_spacing: 16,
                set_hexpand: true,

            #[name = "transport_box"]
            gtk::Box {
                set_spacing: 6,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_icon_name: "media-skip-backward-symbolic",
                    set_action_name: Some(shortcuts::PREVIOUS.action),
                    set_tooltip_text: Some(&shortcuts::PREVIOUS.tooltip(&gettext("Previous"))),
                    add_css_class: "circular",
                    add_css_class: "flat",
                },

                gtk::Button {
                    #[watch]
                    set_icon_name: play_pause_icon(model.state.playing),
                    #[watch]
                    set_tooltip_text: Some(&shortcuts::PLAY_PAUSE.tooltip(&play_pause_label(model.state.playing))),
                    set_action_name: Some(shortcuts::PLAY_PAUSE.action),
                    add_css_class: "circular",
                    add_css_class: "suggested-action",
                },

                gtk::Button {
                    set_icon_name: "media-skip-forward-symbolic",
                    set_action_name: Some(shortcuts::NEXT.action),
                    set_tooltip_text: Some(&shortcuts::NEXT.tooltip(&gettext("Next"))),
                    add_css_class: "circular",
                    add_css_class: "flat",
                },
            },

            gtk::Box {
                set_hexpand: true,
                set_spacing: 8,
                set_valign: gtk::Align::Center,

                #[name = "elapsed_label"]
                gtk::Label {
                    set_label: &format_time(Duration::ZERO),
                    add_css_class: "numeric",
                    add_css_class: "dim-label",
                },

                #[name = "seek_scale"]
                gtk::Scale {
                    set_hexpand: true,
                    set_draw_value: false,
                    set_increments: (5.0, 10.0),
                    update_property: &[gtk::accessible::Property::Label(&gettext("Seek"))],
                    #[watch]
                    set_range: (0.0, model.state.duration.as_secs_f64().max(1.0)),
                    #[watch]
                    set_sensitive: !model.state.duration.is_zero(),
                    connect_change_value[sender] => move |_, _, value| {
                        sender.input(PlayerBarInput::SeekMoved(value));
                        Propagation::Proceed
                    },

                    // A legacy controller sees the raw events. A gesture here would
                    // fight with the gestures of the scale.
                    add_controller = gtk::EventControllerLegacy {
                        set_propagation_phase: gtk::PropagationPhase::Capture,
                        connect_event[sender] => move |_, event| {
                            match event.event_type() {
                                EventType::ButtonPress | EventType::TouchBegin => {
                                    sender.input(PlayerBarInput::SeekPressed);
                                }
                                EventType::ButtonRelease
                                | EventType::TouchEnd
                                | EventType::TouchCancel
                                | EventType::GrabBroken => {
                                    sender.input(PlayerBarInput::SeekReleased);
                                }
                                _ => {}
                            }
                            Propagation::Proceed
                        },
                    },
                },

                gtk::Label {
                    #[watch]
                    set_label: &format_time(model.state.duration),
                    add_css_class: "numeric",
                    add_css_class: "dim-label",
                },
            },

            #[name = "extras_box"]
            gtk::Box {
                set_spacing: 6,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_icon_name: "media-playlist-shuffle-symbolic",
                    #[watch]
                    set_css_classes: accent_when(model.state.shuffle),
                    set_action_name: Some(shortcuts::TOGGLE_SHUFFLE.action),
                    set_tooltip_text: Some(&shortcuts::TOGGLE_SHUFFLE.tooltip(&gettext("Shuffle"))),
                },

                gtk::Button {
                    #[watch]
                    set_icon_name: repeat_icon(model.state.repeat),
                    #[watch]
                    set_css_classes: accent_when(model.state.repeat != RepeatMode::None),
                    #[watch]
                    set_tooltip_text: Some(&shortcuts::CYCLE_REPEAT.tooltip(&repeat_label(model.state.repeat))),
                    set_action_name: Some(shortcuts::CYCLE_REPEAT.action),
                },

                #[name = "queue_button"]
                gtk::Button {
                    set_icon_name: "view-list-symbolic",
                    #[watch]
                    set_css_classes: accent_when(model.queue_open),
                    set_tooltip_text: Some(&TOGGLE_QUEUE.tooltip(&gettext("Queue"))),
                    set_action_name: Some(TOGGLE_QUEUE.action),
                },

                gtk::MenuButton {
                    add_css_class: "flat",
                    set_tooltip_text: Some(&gettext("Volume")),
                    #[watch]
                    set_icon_name: volume_icon(model.state.volume),

                    #[name = "volume_popover"]
                    #[wrap(Some)]
                    set_popover = &gtk::Popover {
                        connect_closed[sender] => move |_| {
                            sender.input(PlayerBarInput::VolumePopoverClosed);
                        },

                        #[wrap(Some)]
                        #[name = "volume_scale"]
                        set_child = &gtk::Scale {
                            set_orientation: gtk::Orientation::Vertical,
                            set_inverted: true,
                            set_draw_value: false,
                            set_range: (0.0, 1.0),
                            set_increments: (0.05, 0.1),
                            set_height_request: 150,
                            update_property: &[gtk::accessible::Property::Label(&gettext("Volume"))],
                            connect_change_value[sender] => move |_, _, value| {
                                sender.input(PlayerBarInput::VolumeMoved(value));
                                Propagation::Proceed
                            },
                        },
                    },
                },

                #[local_ref]
                add_menu_widget -> gtk::MenuButton {},
            },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let PlayerBarInit {
            core,
            settings,
            artwork,
            window,
            catalog,
            api,
            compact,
        } = init;
        let actions = PlaybackActions::register(&window, &core);
        let mut state = BarState::of(&core.subscribe().borrow());
        state.fill_links(&catalog);
        actions.sync(state.action_state());
        restore_volume(&core, &settings);
        forward_state(&core, &sender);

        let artwork = ArtworkImage::new(artwork);
        artwork.set_artwork(state.artwork.as_deref(), ARTWORK_SIZE);
        let artwork_widget = artwork.widget().clone();
        tint::follow_playing(&artwork_widget);
        let add_menu = ItemMenu::new();
        add_menu.set_song(state.song_id.clone());
        let add_menu_widget = add_menu.widget().clone();
        let mut model = Self {
            state,
            actions,
            settings,
            core,
            catalog,
            api,
            window,
            asked_links: HashSet::new(),
            artwork,
            add_menu,
            dragging: false,
            seek_target: None,
            queue_open: false,
        };
        model.ask_links(&sender);
        let widgets = view_output!();
        show_elapsed_time(&widgets.seek_scale, &widgets.elapsed_label);
        CompactMode::install(&compact, &root, &widgets);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            PlayerBarInput::State(state) => {
                self.follow(state);
                self.ask_links(&sender);
            }
            PlayerBarInput::SeekPressed => {
                self.dragging = true;
                self.seek_target = None;
            }
            PlayerBarInput::SeekMoved(seconds) => self.move_seek(seconds),
            PlayerBarInput::SeekReleased => self.release_seek(),
            PlayerBarInput::VolumeMoved(volume) => {
                self.send(Message::SetVolume(core_volume(volume)));
            }
            // The view update that follows shows the volume.
            PlayerBarInput::VolumePopoverClosed => {}
            PlayerBarInput::QueueShown(open) => self.queue_open = open,
            PlayerBarInput::OpenAlbum => self.open(OPEN_ALBUM, self.state.album_id.clone()),
            PlayerBarInput::OpenArtist => self.open(OPEN_ARTIST, self.state.artist_id.clone()),
            PlayerBarInput::LinksLoaded(song_id, links) => self.take_links(&song_id, links),
        }
    }

    fn post_view() {
        // The user owns the sliders while they use them.
        if !model.dragging {
            seek_scale.set_value(model.state.position.as_secs_f64());
        }
        if !volume_popover.is_visible() {
            volume_scale.set_value(model.state.volume);
        }
    }
}

impl PlayerBar {
    /// Takes the new state. The position waits while the user holds the seek
    /// bar. A changed volume goes to the settings.
    fn follow(&mut self, mut state: BarState) {
        state.fill_links(&self.catalog);
        self.actions.sync(state.action_state());
        if self.dragging {
            state.position = self.state.position;
        }
        if state.volume.total_cmp(&self.state.volume).is_ne() {
            self.save_volume(state.volume);
        }
        self.artwork
            .set_artwork(state.artwork.as_deref(), ARTWORK_SIZE);
        if state.song_id != self.state.song_id {
            self.add_menu.set_song(state.song_id.clone());
        }
        self.state = state;
    }

    /// Runs the open action with `id`. It does nothing without an ID.
    fn open(&self, action: &str, id: Option<String>) {
        let Some(id) = id else {
            return;
        };
        if let Err(error) = WidgetExt::activate_action(&self.window, action, Some(&id.to_variant()))
        {
            warn!(%error, action, "cannot open the page");
        }
    }

    /// Asks the API for the links of the current song, once, when no view
    /// knew them.
    fn ask_links(&mut self, sender: &ComponentSender<Self>) {
        if !self.state.lacks_links() {
            return;
        }
        let Some(song_id) = self.state.song_id.clone() else {
            return;
        };
        if !self.asked_links.insert(song_id.clone()) {
            return;
        }
        let storefront = detail::storefront(&self.core.subscribe().borrow()).unwrap_or_default();
        let api = Arc::clone(&self.api);
        let input = sender.input_sender().clone();
        relm4::spawn(async move {
            match detail::load_song_links(&api, &storefront, &song_id).await {
                Ok(links) => input.emit(PlayerBarInput::LinksLoaded(song_id, links)),
                Err(error) => warn!(%error, song_id, "cannot load the links of the song"),
            }
        });
    }

    /// Keeps the links of a song. The bar shows them if the song still plays.
    fn take_links(&mut self, song_id: &str, links: SongLinks) {
        self.catalog.remember_links(song_id, links);
        if self.state.song_id.as_deref() == Some(song_id) {
            self.state.fill_links(&self.catalog);
        }
    }

    /// A move of the seek bar. During a drag, the seek happens at the release.
    fn move_seek(&mut self, seconds: f64) {
        let target = Duration::from_secs_f64(seconds.clamp(0.0, self.state.duration.as_secs_f64()));
        if self.dragging {
            self.seek_target = Some(target);
        } else {
            self.seek_to(target);
        }
    }

    fn release_seek(&mut self) {
        self.dragging = false;
        if let Some(target) = self.seek_target.take() {
            self.seek_to(target);
        }
    }

    /// Sends one seek. The bar shows the target until the engine reports the
    /// new position.
    fn seek_to(&mut self, target: Duration) {
        self.state.position = Duration::from_secs(target.as_secs());
        self.send(Message::Seek(target));
    }

    fn save_volume(&self, volume: f64) {
        // ponytail: one write for each step. Debounce if dconf traffic matters.
        if let Err(error) = self.settings.set_volume(volume) {
            warn!(%error, "cannot save the volume");
        }
    }

    fn send(&self, message: Message) {
        let core = self.core.clone();
        relm4::spawn(async move { core.send(message).await });
    }
}

/// The compact layout of the bar. In a narrow window, the song is on the
/// first row and the controls are stacked below it.
struct CompactMode;

impl CompactMode {
    /// Adds the setters of the compact layout to each of `breakpoints`.
    fn install(breakpoints: &[adw::Breakpoint], bar: &gtk::Box, widgets: &PlayerBarWidgets) {
        let vertical = gtk::Orientation::Vertical.to_value();
        let center = gtk::Align::Center.to_value();
        for breakpoint in breakpoints {
            breakpoint.add_setter(bar, "orientation", Some(&vertical));
            breakpoint.add_setter(&widgets.controls, "orientation", Some(&vertical));
            breakpoint.add_setter(&widgets.controls, "spacing", Some(&8.to_value()));
            breakpoint.add_setter(&widgets.transport_box, "halign", Some(&center));
            breakpoint.add_setter(&widgets.extras_box, "halign", Some(&center));
            breakpoint.add_setter(&widgets.info_box, "width-request", Some(&(-1).to_value()));
            breakpoint.add_setter(&widgets.info_box, "hexpand", Some(&true.to_value()));
            breakpoint.add_setter(&widgets.song_box, "hexpand", Some(&true.to_value()));
        }
    }
}

/// Sends the saved volume to the core. The engine does not know it at start.
fn restore_volume(core: &CoreHandle, settings: &Settings) {
    let (core, volume) = (core.clone(), settings.volume());
    relm4::spawn(async move { core.send(Message::SetVolume(core_volume(volume))).await });
}

/// Makes the elapsed label follow the seek bar, also while the user moves it.
fn show_elapsed_time(scale: &gtk::Scale, label: &gtk::Label) {
    scale.connect_value_changed(glib::clone!(
        #[weak]
        label,
        move |scale| label.set_label(&format_time(Duration::from_secs_f64(scale.value())))
    ));
}

/// Sends the changes of the bar state to the component.
fn forward_state(core: &CoreHandle, sender: &ComponentSender<PlayerBar>) {
    let mut snapshots = core.subscribe();
    let input = sender.input_sender().clone();
    relm4::spawn(async move {
        let mut last = BarState::of(&snapshots.borrow_and_update());
        while snapshots.changed().await.is_ok() {
            let state = BarState::of(&snapshots.borrow_and_update());
            if state != last {
                last = state.clone();
                input.emit(PlayerBarInput::State(state));
            }
        }
    });
}

/// The window action that opens and closes the queue panel, with its key.
pub const TOGGLE_QUEUE: shortcuts::Shortcut = shortcuts::Shortcut {
    action: "win.toggle-queue",
    accelerator: "F9",
};

/// Converts the volume of the scale to the volume of the core.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the volume is between 0 and 1, so f32 is precise enough"
)]
fn core_volume(volume: f64) -> f32 {
    volume.clamp(0.0, 1.0) as f32
}

/// Formats a time as `m:ss`, or as `h:mm:ss` from one hour.
pub(crate) fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn play_pause_icon(playing: bool) -> &'static str {
    if playing {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    }
}

fn play_pause_label(playing: bool) -> String {
    if playing {
        gettext("Pause")
    } else {
        gettext("Play")
    }
}

fn repeat_icon(mode: RepeatMode) -> &'static str {
    match mode {
        RepeatMode::One => "media-playlist-repeat-song-symbolic",
        RepeatMode::None | RepeatMode::All => "media-playlist-repeat-symbolic",
    }
}

/// A toggle button has the accent color when its setting is on.
fn accent_when(on: bool) -> &'static [&'static str] {
    if on { &["flat", "accent"] } else { &["flat"] }
}

/// Names the mode that the button sets next, so the tooltip says what a click does.
fn repeat_label(mode: RepeatMode) -> String {
    match mode {
        RepeatMode::None => gettext("Repeat All"),
        RepeatMode::All => gettext("Repeat Current Song"),
        RepeatMode::One => gettext("Turn Repeat Off"),
    }
}

fn volume_icon(volume: f64) -> &'static str {
    if volume <= 0.0 {
        "audio-volume-muted-symbolic"
    } else if volume < 0.34 {
        "audio-volume-low-symbolic"
    } else if volume < 0.67 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

#[cfg(test)]
mod tests {
    use ringo_core::queue::QueueItem;
    use ringo_core::state::NowPlaying;
    use ringo_engine::ItemId;

    use super::*;

    #[test]
    fn format_time_shows_minutes_and_seconds() {
        assert_eq!(format_time(Duration::from_secs(65)), "1:05");
    }

    #[test]
    fn format_time_shows_hours_from_one_hour() {
        assert_eq!(format_time(Duration::from_secs(3_725)), "1:02:05");
    }

    #[test]
    fn format_time_of_zero_is_zero_minutes() {
        assert_eq!(format_time(Duration::ZERO), "0:00");
    }

    #[test]
    fn bar_state_of_empty_queue_has_no_queue() {
        assert!(!BarState::of(&AppState::new()).has_queue);
    }

    #[test]
    fn bar_state_shows_the_queue_song_before_the_engine_plays() {
        let mut state = AppState::new();
        let item = QueueItem {
            id: ItemId::new("1"),
            title: "Title".to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            duration: Duration::from_secs(90),
            artwork_template: None,
        };
        let _ = state.queue.replace(vec![item], 0);
        let bar = BarState::of(&state);
        assert_eq!(
            (bar.title.as_str(), bar.duration, bar.has_queue),
            ("Title", Duration::from_secs(90), true)
        );
    }

    #[test]
    fn bar_state_has_no_album_before_the_engine_plays() {
        assert_eq!(BarState::of(&AppState::new()).album_id, None);
    }

    #[test]
    fn bar_state_takes_the_album_of_the_playing_song() {
        let mut state = AppState::new();
        state.now_playing = Some(NowPlaying {
            id: ItemId::new("1"),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: Duration::ZERO,
            artwork_url: None,
            album_id: Some("77".to_owned()),
        });
        assert_eq!(BarState::of(&state).album_id.as_deref(), Some("77"));
    }

    #[test]
    fn volume_icon_follows_the_level() {
        let icons = [0.0, 0.2, 0.5, 1.0].map(volume_icon);
        assert_eq!(
            icons,
            [
                "audio-volume-muted-symbolic",
                "audio-volume-low-symbolic",
                "audio-volume-medium-symbolic",
                "audio-volume-high-symbolic"
            ]
        );
    }
}
