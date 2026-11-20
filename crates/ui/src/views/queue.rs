// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Queue page: the item that plays now, the upcoming items in playback
//! order, and the history. Each edit goes to the core as a message. The
//! page follows the queue in the core state and changes only the rows that
//! differ.

use std::collections::HashMap;

use gettextrs::{gettext, ngettext};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gdk::{Key, ModifierType};
use relm4::gtk::gio::{self, ActionEntry};
use relm4::gtk::glib::{Propagation, Variant, VariantTy};
use relm4::prelude::*;
use relm4::typed_view::list::TypedListView;
use ringo_core::queue::{Queue, QueueItem};
use ringo_core::runner::CoreHandle;
use ringo_core::state::Message;
use ringo_engine::ItemId;

use super::queue_diff::{self, Edit};
use crate::views::section_page;
use crate::widgets::artwork_image::ArtworkImage;
use crate::widgets::player_bar::format_time;
use crate::widgets::queue_row::{QueueRow, QueueRowWidgets};

/// What the queue page needs to start.
pub struct QueueInit {
    pub core: CoreHandle,
    pub window: adw::ApplicationWindow,
    pub split_view: adw::NavigationSplitView,
}

/// An item of the queue with its place in the user order.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    /// Index in the user order. The core messages use it.
    index: usize,
    item: QueueItem,
}

/// The data that the page shows. Building it does not touch any widget.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Snapshot {
    now: Option<QueueItem>,
    /// Playback position of the current item.
    current_position: usize,
    upcoming: Vec<Entry>,
    /// The latest played item first. Items that left the queue are not here.
    history: Vec<Entry>,
}

impl Snapshot {
    fn of(queue: &Queue) -> Self {
        let items = queue.items();
        let current_position = queue.current_playback_index();
        let upcoming = current_position
            .map(|current| {
                (current + 1..items.len())
                    .filter_map(|position| queue.index_at_position(position))
                    .map(|index| Entry {
                        index,
                        item: items[index].clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut index_of: HashMap<&ItemId, usize> = HashMap::new();
        for (index, item) in items.iter().enumerate() {
            index_of.entry(&item.id).or_insert(index);
        }
        let played: Vec<&ItemId> = queue.history().collect();
        let history = played
            .into_iter()
            .rev()
            .filter_map(|id| index_of.get(id).copied())
            .map(|index| Entry {
                index,
                item: items[index].clone(),
            })
            .collect();
        Self {
            now: queue.current().cloned(),
            current_position: current_position.unwrap_or_default(),
            upcoming,
            history,
        }
    }
}

/// Input of the queue page.
#[derive(Debug)]
pub enum QueueInput {
    /// The queue in the core state changed.
    Changed(Box<Snapshot>),
    /// The user activated the upcoming item at the position.
    Activated(u32),
    /// The user removes the upcoming item at the position.
    Remove(u32),
    /// The user moves an upcoming item to another position.
    Move { from: u32, to: u32 },
    /// The user moves the selected upcoming item up (`-1`) or down (`1`).
    MoveSelected(i32),
    /// The user removes the selected upcoming item.
    RemoveSelected,
    /// The user activated the history item at the position.
    HistoryActivated(u32),
    /// The user asks to clear the queue.
    ClearRequested,
}

/// The queue page.
pub struct QueueView {
    core: CoreHandle,
    window: adw::ApplicationWindow,
    upcoming: TypedListView<QueueRow, gtk::SingleSelection>,
    now_row: QueueRowWidgets,
    history_expander: adw::ExpanderRow,
    /// The rows of the history. The artwork loads in the background, so the
    /// images live as long as the rows.
    history_rows: Vec<(adw::ActionRow, ArtworkImage)>,
    /// The IDs of the rows of the upcoming list.
    shown: Vec<ItemId>,
    snapshot: Snapshot,
}

#[relm4::component(pub)]
impl SimpleComponent for QueueView {
    type Init = QueueInit;
    type Input = QueueInput;
    type Output = ();

    view! {
        adw::ToolbarView {
            add_top_bar = &adw::HeaderBar {
                pack_start: &back_button,

                #[wrap(Some)]
                set_title_widget = &adw::WindowTitle {
                    set_title: &gettext("Queue"),
                },

                pack_end = &gtk::Button {
                    set_icon_name: "edit-clear-all-symbolic",
                    set_tooltip_text: Some(&gettext("Clear Queue")),
                    set_action_name: Some(CLEAR_ACTION),
                },
            },

            #[wrap(Some)]
            set_content = &gtk::Stack {
                #[watch]
                set_visible_child_name: if model.snapshot.now.is_some() { "content" } else { "empty" },

                add_named[Some("empty")] = &adw::StatusPage {
                    set_icon_name: Some("view-list-symbolic"),
                    set_title: &gettext("The queue is empty"),
                    set_description: Some(&gettext("Songs that you play or add show here.")),
                },

                add_named[Some("content")] = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 24,
                    set_margin_all: 18,

                    append = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 8,

                        append = &section_heading(&gettext("Now Playing")),
                        append = &gtk::Box {
                            add_css_class: "card",
                            append: now_root,
                        },
                    },

                    append = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 8,
                        set_vexpand: true,

                        append = &section_heading(&gettext("Up Next")),
                        append = &gtk::Label {
                            set_label: &gettext("Nothing is up next."),
                            set_xalign: 0.0,
                            set_margin_start: 12,
                            add_css_class: "dim-label",
                            #[watch]
                            set_visible: model.snapshot.upcoming.is_empty(),
                        },
                        append = &gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            add_css_class: "card",
                            #[watch]
                            set_visible: !model.snapshot.upcoming.is_empty(),
                            set_child: Some(&model.upcoming.view),
                        },
                    },

                    append = &gtk::ScrolledWindow {
                        set_propagate_natural_height: true,
                        set_max_content_height: 320,
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                        #[watch]
                        set_visible: !model.snapshot.history.is_empty(),

                        #[wrap(Some)]
                        set_child = &gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::None,
                            add_css_class: "boxed-list",
                            append: &history_expander,
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
        let QueueInit {
            core,
            window,
            split_view,
        } = init;
        let back_button = section_page::back_button(&split_view);
        let upcoming = TypedListView::<QueueRow, gtk::SingleSelection>::new();
        let input = sender.input_sender().clone();
        upcoming
            .view
            .connect_activate(move |_, position| input.emit(QueueInput::Activated(position)));
        connect_keys(&upcoming.view, &sender);
        let (now_root, now_row) = QueueRowWidgets::build();
        let now_root = &now_root;
        let history_expander = adw::ExpanderRow::builder()
            .title(gettext("History"))
            .build();
        register_actions(&window, &sender);
        forward_queue(&core, &sender);

        let model = Self {
            core,
            window,
            upcoming,
            now_row,
            history_expander: history_expander.clone(),
            history_rows: Vec::new(),
            shown: Vec::new(),
            snapshot: Snapshot::default(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            QueueInput::Changed(snapshot) => self.show(*snapshot, &sender),
            QueueInput::Activated(position) => {
                if let Some(entry) = self.upcoming_entry(position) {
                    self.send(Message::JumpTo {
                        index: entry.index,
                        id: entry.item.id.clone(),
                    });
                }
            }
            QueueInput::Remove(position) => self.remove_upcoming(position),
            QueueInput::Move { from, to } => self.move_upcoming(from, to),
            QueueInput::MoveSelected(step) => {
                let selected = self.upcoming.selection_model.selected();
                let target = selected.checked_add_signed(step);
                if let Some(target) = target {
                    self.move_upcoming(selected, target);
                }
            }
            QueueInput::RemoveSelected => {
                self.remove_upcoming(self.upcoming.selection_model.selected());
            }
            QueueInput::HistoryActivated(position) => {
                let entry = usize::try_from(position)
                    .ok()
                    .and_then(|position| self.snapshot.history.get(position));
                if let Some(entry) = entry {
                    self.send(Message::JumpTo {
                        index: entry.index,
                        id: entry.item.id.clone(),
                    });
                }
            }
            QueueInput::ClearRequested => self.confirm_clear(),
        }
    }
}

/// Size of the artwork of a history row in pixels.
const HISTORY_ARTWORK_SIZE: i32 = 40;

/// Name of the window action that asks to clear the queue.
const CLEAR_ACTION: &str = "win.queue-clear";

impl QueueView {
    fn upcoming_entry(&self, position: u32) -> Option<&Entry> {
        self.snapshot.upcoming.get(usize::try_from(position).ok()?)
    }

    fn remove_upcoming(&self, position: u32) {
        if let Some(entry) = self.upcoming_entry(position) {
            self.send(Message::Remove {
                index: entry.index,
                id: entry.item.id.clone(),
            });
        }
    }

    /// Sends a move to the core. The positions are in the upcoming list.
    fn move_upcoming(&self, from: u32, to: u32) {
        let count = self.snapshot.upcoming.len();
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        if from >= count || to >= count || from == to {
            return;
        }
        // The upcoming list starts after the current item.
        let first = self.snapshot.current_position + 1;
        self.send(Message::MoveUpcoming {
            id: self.snapshot.upcoming[from].item.id.clone(),
            from: first + from,
            to: first + to,
        });
    }

    fn send(&self, message: Message) {
        let core = self.core.clone();
        relm4::spawn(async move { core.send(message).await });
    }

    /// Shows `snapshot`. Only the rows that differ change.
    fn show(&mut self, snapshot: Snapshot, sender: &ComponentSender<Self>) {
        if let Some(now) = &snapshot.now {
            self.now_row.show(now);
        }
        self.show_upcoming(&snapshot.upcoming);
        if snapshot.history != self.snapshot.history {
            self.show_history(&snapshot.history, sender);
        }
        self.snapshot = snapshot;
    }

    fn show_upcoming(&mut self, want: &[Entry]) {
        let want_ids: Vec<ItemId> = want.iter().map(|entry| entry.item.id.clone()).collect();
        if want_ids == self.shown {
            return;
        }
        let row = |entry: &Entry| QueueRow::new(entry.item.clone());
        match queue_diff::single_edit(&self.shown, &want_ids) {
            Some(Edit::Remove(position)) => self.upcoming.remove(to_u32(position)),
            Some(Edit::Insert(position)) => {
                self.upcoming.insert(to_u32(position), row(&want[position]));
            }
            Some(Edit::Move { from, to }) => {
                let selected = self.upcoming.selection_model.selected();
                self.upcoming.remove(to_u32(from));
                self.upcoming.insert(to_u32(to), row(&want[to]));
                // The moved row stays selected: the keys can move it again.
                if selected == to_u32(from) {
                    self.upcoming.selection_model.set_selected(to_u32(to));
                }
            }
            None => {
                self.upcoming.clear();
                self.upcoming.extend_from_iter(want.iter().map(row));
            }
        }
        self.shown = want_ids;
    }

    fn show_history(&mut self, history: &[Entry], sender: &ComponentSender<Self>) {
        for (row, _) in self.history_rows.drain(..) {
            self.history_expander.remove(&row);
        }
        let count = u32::try_from(history.len()).unwrap_or(u32::MAX);
        self.history_expander.set_subtitle(
            &ngettext("{} song", "{} songs", count).replace("{}", &count.to_string()),
        );
        for (position, entry) in (0..).zip(history) {
            let (row, artwork) = history_row(&entry.item, position, sender);
            self.history_expander.add_row(&row);
            self.history_rows.push((row, artwork));
        }
    }

    /// Asks the user, then clears the queue.
    fn confirm_clear(&self) {
        if self.snapshot.now.is_none() {
            return;
        }
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Clear Queue?"))
            .body(gettext("All songs leave the queue and playback stops."))
            .close_response("cancel")
            .default_response("cancel")
            .build();
        dialog.add_responses(&[
            ("cancel", &gettext("_Cancel")),
            ("clear", &gettext("_Clear")),
        ]);
        dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
        let core = self.core.clone();
        dialog.choose(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |response| {
                if response == "clear" {
                    relm4::spawn(async move { core.send(Message::ClearQueue).await });
                }
            },
        );
    }
}

fn to_u32(position: usize) -> u32 {
    u32::try_from(position).unwrap_or(u32::MAX)
}

fn section_heading(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .margin_start(12)
        .css_classes(["heading"])
        .build()
}

/// Builds the row of a history item. A press on it plays the item again.
fn history_row(
    item: &QueueItem,
    position: u32,
    sender: &ComponentSender<QueueView>,
) -> (adw::ActionRow, ArtworkImage) {
    let row = adw::ActionRow::builder()
        .title(&item.title)
        .subtitle(&item.artist)
        .use_markup(false)
        .activatable(true)
        .build();
    let artwork = ArtworkImage::shared();
    artwork.set_artwork(item.artwork_template.as_deref(), HISTORY_ARTWORK_SIZE);
    row.add_prefix(artwork.widget());
    row.add_suffix(
        &gtk::Label::builder()
            .label(format_time(item.duration))
            .css_classes(["dim-label", "numeric"])
            .build(),
    );
    let input = sender.input_sender().clone();
    row.connect_activated(move |_| input.emit(QueueInput::HistoryActivated(position)));
    (row, artwork)
}

/// Connects the keys of the upcoming list: Delete removes the selected
/// item, and Alt+Up and Alt+Down move it.
fn connect_keys(view: &gtk::ListView, sender: &ComponentSender<QueueView>) {
    let input = sender.input_sender().clone();
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, state| {
        let alt = state.contains(ModifierType::ALT_MASK);
        let message = match key {
            Key::Delete => QueueInput::RemoveSelected,
            Key::Up if alt => QueueInput::MoveSelected(-1),
            Key::Down if alt => QueueInput::MoveSelected(1),
            _ => return Propagation::Proceed,
        };
        input.emit(message);
        Propagation::Stop
    });
    view.add_controller(keys);
}

/// Adds the window actions of the queue. The rows and the header bar call
/// them.
fn register_actions(window: &adw::ApplicationWindow, sender: &ComponentSender<QueueView>) {
    let remove_input = sender.input_sender().clone();
    let remove = ActionEntry::builder("queue-remove")
        .parameter_type(Some(VariantTy::UINT32))
        .activate(move |_: &adw::ApplicationWindow, _, parameter| {
            if let Some(position) = parameter.and_then(Variant::get::<u32>) {
                remove_input.emit(QueueInput::Remove(position));
            }
        })
        .build();
    let move_input = sender.input_sender().clone();
    let move_item = ActionEntry::builder("queue-move")
        .parameter_type(Some(
            VariantTy::new("(uu)").expect("the type string is valid"),
        ))
        .activate(move |_: &adw::ApplicationWindow, _, parameter| {
            if let Some((from, to)) = parameter.and_then(Variant::get::<(u32, u32)>) {
                move_input.emit(QueueInput::Move { from, to });
            }
        })
        .build();
    let clear_input = sender.input_sender().clone();
    let clear = ActionEntry::builder("queue-clear")
        .activate(move |_: &adw::ApplicationWindow, _, _| {
            clear_input.emit(QueueInput::ClearRequested);
        })
        .build();
    window.add_action_entries([remove, move_item, clear]);
}

/// Sends a snapshot each time the queue changes. Position and volume
/// changes of the state do not wake the main thread.
fn forward_queue(core: &CoreHandle, sender: &ComponentSender<QueueView>) {
    let mut state = core.subscribe();
    let input = sender.input_sender().clone();
    relm4::spawn(async move {
        let mut last: Option<Queue> = None;
        loop {
            let snapshot = {
                let current = state.borrow_and_update();
                (last.as_ref() != Some(&current.queue)).then(|| {
                    last = Some(current.queue.clone());
                    Snapshot::of(&current.queue)
                })
            };
            if let Some(snapshot) = snapshot {
                input.emit(QueueInput::Changed(Box::new(snapshot)));
            }
            if state.changed().await.is_err() {
                return;
            }
        }
    });
}
