// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The favorite star of an item: one stateful action, shared by the star button and the
//! "Favorite" menu items (decisions L7, L8, L14, L18, and L20).
//!
//! The action state is the optimistic value. A click changes it at once and sends one request.
//! A failed request changes it back. The status is read live when the item shows, and again after
//! each successful write until Apple confirms it (`rating_confirmation.rs`).
//!
//! Apple documents no way to remove an artist favorite. So the star of an artist that is a
//! favorite is disabled, and its tooltip sends the user to Apple's apps.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::gio::{SimpleAction, SimpleActionGroup};
use relm4::gtk::glib::{self, Variant};
use relm4::gtk::prelude::*;
use ringo_am_api::write::{self, CatalogItem};
use tracing::warn;

use crate::library_write;
use crate::rating_confirmation::{self, RatingControl, RatingState, WriteToken};
use crate::services;
use crate::status_batch::{StatusBatch, StatusTarget};

/// Name of the favorite action, in every group that holds it.
pub const ACTION_NAME: &str = "favorite";

/// Name of the action group of a star button.
const BUTTON_GROUP: &str = "star";

/// What the star of one item shows.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Look {
    /// The item is a favorite (the optimistic value).
    pub favorite: bool,
    /// The item is an artist. The app cannot remove an artist favorite.
    pub artist: bool,
}

impl Look {
    /// Tells if a click can change the star. Only the removal of an artist favorite is not
    /// possible.
    fn can_toggle(self) -> bool {
        !(self.favorite && self.artist)
    }

    fn icon_name(self) -> &'static str {
        if self.favorite {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        }
    }

    fn tooltip(self) -> String {
        match (self.favorite, self.artist) {
            (false, _) => gettext("Add to favorites"),
            (true, false) => gettext("Remove from favorites"),
            (true, true) => gettext(
                "Remove this artist from your favorites in the Apple Music app or on music.apple.com",
            ),
        }
    }
}

/// What the control knows about the favorite status of its item.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
enum Status {
    /// No item, or no status request yet. The star is off.
    #[default]
    Unknown,
    /// A request reads the status. The star is off.
    Loading,
    /// The status. After a click it is the optimistic value.
    Known(bool),
}

/// One click that changed the status and sent a request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Toggle {
    generation: u64,
    pub(crate) item: CatalogItem,
    pub(crate) target: bool,
}

/// A change of the star that a linked control must know (see [`FavoriteControl::connect_change`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Change {
    /// A click set the star to this value and sent a request.
    Toggled(bool),
    /// The request of a click failed, so the star went back from this value.
    Undone(bool),
    /// After a write, Apple has this other state for `item`. The star already shows it.
    Corrected {
        item: CatalogItem,
        state: RatingState,
    },
}

/// The state logic of the star, with no widget. A generation number drops the result of a request
/// for an item that is no longer shown. The dislike control of `dislike.rs` reuses it for a
/// boolean status.
#[derive(Debug, Default)]
pub(crate) struct FavoriteState {
    item: Option<CatalogItem>,
    status: Status,
    generation: u64,
}

impl FavoriteState {
    /// Shows `item` with an unknown status. Results of earlier requests are dropped.
    pub(crate) fn reset(&mut self, item: Option<CatalogItem>) {
        self.generation += 1;
        self.item = item;
        self.status = Status::Unknown;
    }

    /// Starts a status read. Returns the request, or `None` without an item.
    pub(crate) fn begin_load(&mut self) -> Option<(u64, CatalogItem)> {
        let item = self.item.clone()?;
        self.generation += 1;
        self.status = Status::Loading;
        Some((self.generation, item))
    }

    /// Takes the result of a status read. Returns `false` if the result is out of date.
    pub(crate) fn finish_load(&mut self, generation: u64, favorite: bool) -> bool {
        let current = generation == self.generation && self.status == Status::Loading;
        if current {
            self.status = Status::Known(favorite);
        }
        current
    }

    /// Changes the status to `target` before the request. Returns `None` if the status is not
    /// known or already has this value.
    pub(crate) fn begin_toggle(&mut self, target: bool) -> Option<Toggle> {
        let item = self.item.clone()?;
        if self.status != Status::Known(!target) {
            return None;
        }
        self.status = Status::Known(target);
        Some(Toggle {
            generation: self.generation,
            item,
            target,
        })
    }

    /// Changes the status back after a failed request. Returns `false` if the item changed, or a
    /// newer click already changed the status again.
    pub(crate) fn undo(&mut self, toggle: &Toggle) -> bool {
        let current =
            toggle.generation == self.generation && self.status == Status::Known(toggle.target);
        if current {
            self.status = Status::Known(!toggle.target);
        }
        current
    }

    /// Tells if a status read of `generation` is still the one that the state waits for.
    pub(crate) fn is_loading(&self, generation: u64) -> bool {
        generation == self.generation && self.status == Status::Loading
    }

    pub(crate) fn is_known(&self) -> bool {
        matches!(self.status, Status::Known(_))
    }

    /// Tells if the status is known and on.
    pub(crate) fn is_on(&self) -> bool {
        self.status == Status::Known(true)
    }

    /// Sets a known status with no request, because another control decided it. Does nothing
    /// while the status is not known.
    pub(crate) fn force(&mut self, value: bool) {
        if self.is_known() {
            self.status = Status::Known(value);
        }
    }

    /// Sets a known status with no request, like [`Self::force`], but only while the state shows
    /// `item`. Returns `true` if the status changed.
    pub(crate) fn force_for(&mut self, item: &CatalogItem, value: bool) -> bool {
        let applies = self.item.as_ref() == Some(item) && self.status == Status::Known(!value);
        if applies {
            self.status = Status::Known(value);
        }
        applies
    }

    fn look(&self) -> Look {
        Look {
            favorite: self.status == Status::Known(true),
            artist: matches!(self.item, Some(CatalogItem::Artist(_))),
        }
    }
}

/// Callback that shows a [`Look`].
type LookHandler = Box<dyn Fn(Look)>;

/// Callback that learns a [`Change`].
type ChangeHandler = Box<dyn Fn(Change)>;

struct Inner {
    action: SimpleAction,
    state: RefCell<FavoriteState>,
    /// A widget of the view. The toast of a failed write appears over its window.
    anchor: glib::WeakRef<gtk::Widget>,
    on_look: RefCell<Option<LookHandler>>,
    on_change: RefCell<Option<ChangeHandler>>,
}

impl Inner {
    /// Puts the state in the action and tells the look to the star button.
    fn sync(&self) {
        let (known, look) = {
            let state = self.state.borrow();
            (state.is_known(), state.look())
        };
        self.action.set_enabled(known && look.can_toggle());
        self.action.set_state(&look.favorite.to_variant());
        if let Some(on_look) = self.on_look.borrow().as_ref() {
            on_look(look);
        }
    }

    /// Handles a click or a menu choice: changes the status at once, then sends one request.
    /// After a successful request, Apple confirms the new status.
    fn toggle(self: &Rc<Self>, target: bool) {
        let Some(toggle) = self.state.borrow_mut().begin_toggle(target) else {
            return;
        };
        self.sync();
        self.notify(Change::Toggled(target));
        let (Some(anchor), Some(services)) = (self.anchor.upgrade(), services::installed()) else {
            warn!("the favorite has no widget or no services");
            self.undo(&toggle);
            return;
        };
        let token = WriteToken::begin(&toggle.item);
        let api = Arc::clone(services.api());
        let item = toggle.item.clone();
        let request = async move {
            if target {
                write::add_to_favorites(api.as_ref(), &item).await
            } else {
                write::remove_rating(api.as_ref(), &item).await
            }
        };
        let expected = if target {
            RatingState::Favorite
        } else {
            RatingState::Unrated
        };
        let (confirming, undoing) = (Rc::downgrade(self), Rc::clone(self));
        library_write::run(
            &anchor,
            request,
            move |()| rating_confirmation::confirm(&confirming, token, expected),
            move || undoing.undo(&toggle),
        );
    }

    fn undo(&self, toggle: &Toggle) {
        let changed = self.state.borrow_mut().undo(toggle);
        if changed {
            self.sync();
            self.notify(Change::Undone(toggle.target));
        }
    }

    fn notify(&self, change: Change) {
        if let Some(on_change) = self.on_change.borrow().as_ref() {
            on_change(change);
        }
    }
}

impl RatingControl for Inner {
    fn anchor(&self) -> Option<gtk::Widget> {
        self.anchor.upgrade()
    }

    /// Shows the favorite status of `state` on the star, and tells the linked control.
    fn correct(&self, item: &CatalogItem, state: RatingState) {
        let changed = self.state.borrow_mut().force_for(item, state.is_favorite());
        if changed {
            self.sync();
        }
        self.notify(Change::Corrected {
            item: item.clone(),
            state,
        });
    }
}

/// The favorite state of one item, as a boolean action. Clones share the state.
#[derive(Clone)]
pub struct FavoriteControl {
    inner: Rc<Inner>,
}

impl FavoriteControl {
    /// Makes a control with no item. Its action is off. `anchor` is a widget of the view.
    pub fn new(anchor: &impl IsA<gtk::Widget>) -> Self {
        let action = SimpleAction::new_stateful(ACTION_NAME, None, &false.to_variant());
        action.set_enabled(false);
        let inner = Rc::new(Inner {
            action,
            state: RefCell::default(),
            anchor: anchor.as_ref().downgrade(),
            on_look: RefCell::default(),
            on_change: RefCell::default(),
        });
        let weak: Weak<Inner> = Rc::downgrade(&inner);
        inner.action.connect_change_state(move |_, value| {
            let target = value.and_then(Variant::get::<bool>);
            if let (Some(inner), Some(target)) = (weak.upgrade(), target) {
                inner.toggle(target);
            }
        });
        Self { inner }
    }

    /// The action. Add it to the group of a menu button, or of a row, for the "Favorite" item.
    pub fn action(&self) -> &SimpleAction {
        &self.inner.action
    }

    /// Shows `item` and reads its status. The star waits for the result. A result for an earlier
    /// item is dropped.
    pub fn show_item(&self, item: Option<CatalogItem>) {
        self.set_item(item);
        self.load_status();
    }

    /// Shows `item` with an unknown status and no request. Call [`Self::load_status`] or
    /// [`Self::queue_status`] to read it.
    pub fn set_item(&self, item: Option<CatalogItem>) {
        self.inner.state.borrow_mut().reset(item);
        self.inner.sync();
    }

    /// Reads the status together with the other controls that queue in the same moment. The
    /// rows of a list use this: they get one request for many songs, not one request for each
    /// row.
    pub fn queue_status(&self) {
        if let Some((generation, item)) = self.begin_load() {
            StatusBatch::queue(&READS, self.target(), generation, item);
        }
    }

    /// Reads the status of the item, live, and shows it.
    pub fn load_status(&self) {
        if let Some((generation, item)) = self.begin_load() {
            StatusBatch::read_now(&READS, self.target(), generation, item);
        }
    }

    /// Starts a read and shows that the status loads. `None` if no read is necessary.
    fn begin_load(&self) -> Option<(u64, CatalogItem)> {
        let request = self.inner.state.borrow_mut().begin_load();
        if request.is_some() {
            self.inner.sync();
        }
        request
    }

    fn target(&self) -> Weak<dyn StatusTarget> {
        Rc::downgrade(&self.inner) as Weak<dyn StatusTarget>
    }

    /// Tells if the status is known and on.
    pub(crate) fn is_on(&self) -> bool {
        self.inner.state.borrow().is_on()
    }

    /// Sets a known status with no request. Use it when another control already sent the request
    /// that changes the favorite (a dislike clears it).
    pub(crate) fn set_on_quietly(&self, value: bool) {
        self.inner.state.borrow_mut().force(value);
        self.inner.sync();
    }

    /// Shows the favorite status that Apple has for `item` after a write of another control. Does
    /// nothing if the control shows another item.
    pub(crate) fn show_confirmed(&self, item: &CatalogItem, favorite: bool) {
        let changed = self.inner.state.borrow_mut().force_for(item, favorite);
        if changed {
            self.inner.sync();
        }
    }

    /// Calls `on_change` for each click, each undo of a click, and each correction after a write.
    /// One handler at a time.
    pub(crate) fn connect_change(&self, on_change: impl Fn(Change) + 'static) {
        *self.inner.on_change.borrow_mut() = Some(Box::new(on_change));
    }

    fn connect_look(&self, on_look: impl Fn(Look) + 'static) {
        on_look(self.inner.state.borrow().look());
        *self.inner.on_look.borrow_mut() = Some(Box::new(on_look));
    }
}

impl StatusTarget for Inner {
    fn is_loading(&self, generation: u64) -> bool {
        self.state.borrow().is_loading(generation)
    }

    fn finish_load(&self, generation: u64, favorite: bool) {
        let current = self.state.borrow_mut().finish_load(generation, favorite);
        if current {
            self.sync();
        }
    }
}

thread_local! {
    /// The status reads of the stars. A failure counts as "not a favorite": the star stays
    /// usable, and Apple ignores a duplicate add.
    static READS: StatusBatch = const { StatusBatch::new(|items| Box::pin(read_statuses(items))) };
}

/// Reads the status of `items`, live. After a failure the result is empty.
async fn read_statuses(items: Vec<CatalogItem>) -> HashMap<CatalogItem, bool> {
    let Some(services) = services::installed() else {
        return HashMap::new();
    };
    let storefront = services.core().storefront().unwrap_or_default();
    let api = Arc::clone(services.api());
    let task =
        relm4::spawn(
            async move { write::favorite_status(api.as_ref(), &storefront, &items).await },
        );
    match task.await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            warn!(%error, "cannot read the favorite status");
            HashMap::new()
        }
        Err(join_error) => {
            warn!(%join_error, "the favorite status task did not finish");
            HashMap::new()
        }
    }
}

/// What decides if a star button shows.
#[derive(Default)]
struct Appearance {
    look: Cell<Option<Look>>,
    /// The pointer is over the place of the star.
    hovered: Cell<bool>,
    /// The star shows only while hovered, or when the item is a favorite.
    idle_hidden: Cell<bool>,
}

impl Appearance {
    fn set_hovered(&self, button: &glib::WeakRef<gtk::Button>, hovered: bool) {
        self.hovered.set(hovered);
        if let Some(button) = button.upgrade() {
            self.apply(&button);
        }
    }

    fn apply(&self, button: &gtk::Button) {
        let favorite = self.look.get().is_some_and(|look| look.favorite);
        let shown = !self.idle_hidden.get() || self.hovered.get() || favorite;
        button.set_visible(shown);
    }
}

/// The star button of an item.
pub struct FavoriteButton {
    button: gtk::Button,
    control: FavoriteControl,
    appearance: Rc<Appearance>,
}

impl FavoriteButton {
    /// Makes a star button with no item. It is insensitive until the status is known.
    pub fn new() -> Self {
        let button = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        let control = FavoriteControl::new(&button);
        let group = SimpleActionGroup::new();
        group.add_action(control.action());
        button.insert_action_group(BUTTON_GROUP, Some(&group));
        button.set_action_name(Some(&format!("{BUTTON_GROUP}.{ACTION_NAME}")));
        let appearance = Rc::new(Appearance::default());
        let (weak, shared) = (button.downgrade(), Rc::clone(&appearance));
        control.connect_look(move |look| {
            let Some(button) = weak.upgrade() else {
                return;
            };
            button.set_icon_name(look.icon_name());
            button.set_tooltip_text(Some(&look.tooltip()));
            shared.look.set(Some(look));
            shared.apply(&button);
        });
        Self {
            button,
            control,
            appearance,
        }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::Button {
        &self.button
    }

    /// The control. The menus of the same item use its action.
    pub fn control(&self) -> &FavoriteControl {
        &self.control
    }

    /// Shows the star only while the pointer is over `row`, or when the item is a favorite. A
    /// hidden star takes no space.
    pub fn show_on_hover_of(&self, row: &impl IsA<gtk::Widget>) {
        self.appearance.idle_hidden.set(true);
        self.appearance.apply(&self.button);
        let hover = gtk::EventControllerMotion::new();
        let (weak, shared) = (self.button.downgrade(), Rc::clone(&self.appearance));
        hover.connect_enter(move |_, _, _| shared.set_hovered(&weak, true));
        let (weak, shared) = (self.button.downgrade(), Rc::clone(&self.appearance));
        hover.connect_leave(move |_| shared.set_hovered(&weak, false));
        row.add_controller(hover);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str) -> CatalogItem {
        CatalogItem::Song(id.to_owned().into())
    }

    fn known(item: CatalogItem, favorite: bool) -> FavoriteState {
        let mut state = FavoriteState::default();
        state.reset(Some(item));
        let (generation, _) = state.begin_load().expect("the state has an item");
        assert!(state.finish_load(generation, favorite));
        state
    }

    #[test]
    fn toggle_changes_the_status_before_the_request() {
        let mut state = known(song("1"), false);
        let toggle = state.begin_toggle(true).expect("the status is known");
        assert!(toggle.target);
        assert!(state.look().favorite);
    }

    #[test]
    fn undo_changes_the_status_back_after_a_failure() {
        let mut state = known(song("1"), false);
        let toggle = state.begin_toggle(true).expect("the status is known");
        assert!(state.undo(&toggle));
        assert!(!state.look().favorite);
    }

    #[test]
    fn toggle_is_refused_while_the_status_loads() {
        let mut state = FavoriteState::default();
        state.reset(Some(song("1")));
        state.begin_load();
        assert_eq!(state.begin_toggle(true), None);
    }

    #[test]
    fn toggle_is_refused_without_an_item() {
        let mut state = FavoriteState::default();
        state.reset(None);
        assert_eq!(state.begin_toggle(true), None);
        assert_eq!(state.begin_load(), None);
    }

    #[test]
    fn a_status_of_the_old_item_is_dropped() {
        let mut state = FavoriteState::default();
        state.reset(Some(song("old")));
        let (old_generation, _) = state.begin_load().expect("the state has an item");
        state.reset(Some(song("new")));
        let (new_generation, _) = state.begin_load().expect("the state has an item");
        assert!(!state.finish_load(old_generation, true));
        assert!(!state.look().favorite);
        assert!(state.finish_load(new_generation, false));
    }

    #[test]
    fn undo_is_dropped_after_the_item_changed() {
        let mut state = known(song("old"), false);
        let toggle = state.begin_toggle(true).expect("the status is known");
        state.reset(Some(song("new")));
        assert!(!state.undo(&toggle));
        assert_eq!(state.status, Status::Unknown);
    }

    #[test]
    fn undo_is_dropped_after_a_newer_click() {
        let mut state = known(song("1"), false);
        let first = state.begin_toggle(true).expect("the status is known");
        let second = state.begin_toggle(false).expect("the status is known");
        assert!(!state.undo(&first));
        assert!(state.undo(&second));
        assert!(state.look().favorite);
    }

    #[test]
    fn tooltip_of_an_active_artist_star_names_apples_apps() {
        let look = Look {
            favorite: true,
            artist: true,
        };
        assert_eq!(
            look.tooltip(),
            "Remove this artist from your favorites in the Apple Music app or on music.apple.com"
        );
        let song = Look {
            favorite: true,
            artist: false,
        };
        assert_eq!(song.tooltip(), "Remove from favorites");
    }

    #[test]
    fn only_an_active_artist_star_cannot_toggle() {
        for (favorite, artist, can_toggle) in [
            (false, false, true),
            (true, false, true),
            (false, true, true),
            (true, true, false),
        ] {
            let look = Look { favorite, artist };
            assert_eq!(look.can_toggle(), can_toggle, "{look:?}");
        }
    }

    #[test]
    fn force_for_changes_only_the_shown_item() {
        let mut state = known(song("1"), false);
        assert!(!state.force_for(&song("2"), true));
        assert!(!state.look().favorite);
        assert!(state.force_for(&song("1"), true));
        assert!(state.look().favorite);
    }

    #[test]
    fn look_of_an_artist_state_is_marked() {
        let mut state = FavoriteState::default();
        state.reset(Some(CatalogItem::Artist("1".to_owned().into())));
        assert!(state.look().artist);
    }
}
