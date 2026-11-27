// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The "Suggest Less" menu item of an item (a dislike for Apple): one stateful action that works
//! with the star of `favorite.rs` (decisions L7, L8, and L20).
//!
//! The star and the dislike exclude each other. A dislike turns the star off, and the star
//! turns a dislike off. Each change sends one request. The state is read live when the menu
//! opens, and again after each successful write until Apple confirms it
//! (`rating_confirmation.rs`). A failed request changes both controls back.
//!
//! Apple reports a new rating late. So the control remembers each change of this session and
//! uses it before it asks Apple. Only songs have a dislike item.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use relm4::gtk;
use relm4::gtk::gio::SimpleAction;
use relm4::gtk::glib::{self, Variant};
use relm4::gtk::prelude::*;
use ringo_am_api::ApiTransport;
use ringo_am_api::write::{self, CatalogItem};
use tracing::warn;

use crate::favorite::{Change, FavoriteControl, FavoriteState};
use crate::library_write;
use crate::rating_confirmation::{self, RatingControl, RatingState, WriteToken};
use crate::services;
use crate::status_batch::{StatusBatch, StatusTarget};

/// Name of the dislike action, in every group that holds it.
pub const ACTION_NAME: &str = "dislike";

thread_local! {
    /// The dislike state that this session set, for each item.
    static SESSION: RefCell<HashMap<CatalogItem, bool>> = RefCell::new(HashMap::new());
}

fn remember(item: Option<&CatalogItem>, disliked: bool) {
    if let Some(item) = item {
        SESSION.with_borrow_mut(|session| session.insert(item.clone(), disliked));
    }
}

fn remembered(item: &CatalogItem) -> Option<bool> {
    SESSION.with_borrow(|session| session.get(item).copied())
}

/// The one request that moves an item from one rating state to another.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RatingRequest {
    AddFavorite,
    SetDislike,
    /// Removes a favorite or a dislike.
    RemoveRating,
}

/// Gives the request that changes `old` to `new`, or `None` if the state does not change. Each
/// change needs at most one request, because Apple replaces the old rating.
fn next_request(old: RatingState, new: RatingState) -> Option<RatingRequest> {
    match (old, new) {
        (RatingState::Unrated, RatingState::Unrated)
        | (RatingState::Favorite, RatingState::Favorite)
        | (RatingState::Dislike, RatingState::Dislike) => None,
        (RatingState::Unrated | RatingState::Dislike, RatingState::Favorite) => {
            Some(RatingRequest::AddFavorite)
        }
        (RatingState::Unrated | RatingState::Favorite, RatingState::Dislike) => {
            Some(RatingRequest::SetDislike)
        }
        (RatingState::Favorite | RatingState::Dislike, RatingState::Unrated) => {
            Some(RatingRequest::RemoveRating)
        }
    }
}

impl RatingRequest {
    async fn send(self, api: &impl ApiTransport, item: &CatalogItem) -> ringo_am_api::Result<()> {
        match self {
            Self::AddFavorite => write::add_to_favorites(api, item).await,
            Self::SetDislike => write::set_dislike(api, item).await,
            Self::RemoveRating => write::remove_rating(api, item).await,
        }
    }
}

/// Callback that learns the dislike state.
type ChangeHandler = Box<dyn Fn(bool)>;

struct Inner {
    action: SimpleAction,
    /// The boolean state "is disliked", with the same rules as the star.
    state: RefCell<FavoriteState>,
    favorite: FavoriteControl,
    /// A widget of the view. The toast of a failed write appears over its window.
    anchor: glib::WeakRef<gtk::Widget>,
    /// A click on the star turned a dislike off. A failed star request turns it on again.
    cleared_by_star: Cell<bool>,
    item: RefCell<Option<CatalogItem>>,
    on_change: RefCell<Option<ChangeHandler>>,
}

impl Inner {
    /// Puts the state in the action.
    fn sync(&self) {
        let (known, disliked) = {
            let state = self.state.borrow();
            (state.is_known(), state.is_on())
        };
        self.action.set_enabled(known);
        self.action.set_state(&disliked.to_variant());
        if let Some(on_change) = self.on_change.borrow().as_ref() {
            on_change(disliked);
        }
    }

    /// Handles a menu choice: changes both controls at once, then sends one request. After a
    /// successful request, Apple confirms the new state.
    fn toggle(self: &Rc<Self>, target: bool) {
        let favorite_on = self.favorite.is_on();
        let (old, new) = if target {
            let old = if favorite_on {
                RatingState::Favorite
            } else {
                RatingState::Unrated
            };
            (old, RatingState::Dislike)
        } else {
            (RatingState::Dislike, RatingState::Unrated)
        };
        let Some(request) = next_request(old, new) else {
            return;
        };
        let Some(toggle) = self.state.borrow_mut().begin_toggle(target) else {
            return;
        };
        let displaced_favorite = target && favorite_on;
        if displaced_favorite {
            self.favorite.set_on_quietly(false);
        }
        remember(Some(&toggle.item), target);
        self.sync();
        let (Some(anchor), Some(services)) = (self.anchor.upgrade(), services::installed()) else {
            warn!("the dislike has no widget or no services");
            self.undo(&toggle, displaced_favorite);
            return;
        };
        let token = WriteToken::begin(&toggle.item);
        let api = Arc::clone(services.api());
        let item = toggle.item.clone();
        let (confirming, undoing) = (Rc::downgrade(self), Rc::clone(self));
        library_write::run(
            &anchor,
            async move { request.send(api.as_ref(), &item).await },
            move |()| rating_confirmation::confirm(&confirming, token, new),
            move || undoing.undo(&toggle, displaced_favorite),
        );
    }

    /// Shows the dislike state that Apple has for `item`. Does nothing to the control if it
    /// shows another item.
    fn show_confirmed(&self, item: &CatalogItem, disliked: bool) {
        remember(Some(item), disliked);
        let changed = self.state.borrow_mut().force_for(item, disliked);
        if changed {
            self.sync();
        }
    }

    /// Changes the dislike back after a failed request. Turns the star on again if the dislike
    /// had turned it off.
    fn undo(&self, toggle: &crate::favorite::Toggle, displaced_favorite: bool) {
        let changed = self.state.borrow_mut().undo(toggle);
        if changed {
            remember(Some(&toggle.item), !toggle.target);
            if displaced_favorite {
                self.favorite.set_on_quietly(true);
            }
            self.sync();
        }
    }

    /// Follows the star: a star that turns on turns a dislike off, a failed star request turns
    /// the dislike on again, and a correction after a star write shows the state of Apple.
    fn follow(&self, change: Change) {
        match change {
            Change::Toggled(on) => {
                let cleared = on && self.state.borrow().is_on();
                if cleared {
                    self.state.borrow_mut().force(false);
                    remember(self.item.borrow().as_ref(), false);
                    self.sync();
                }
                self.cleared_by_star.set(cleared);
            }
            Change::Undone(on) => {
                if on && self.cleared_by_star.replace(false) {
                    self.state.borrow_mut().force(true);
                    remember(self.item.borrow().as_ref(), true);
                    self.sync();
                }
            }
            Change::Corrected { item, state } => self.show_confirmed(&item, state.is_disliked()),
        }
    }
}

impl RatingControl for Inner {
    fn anchor(&self) -> Option<gtk::Widget> {
        self.anchor.upgrade()
    }

    /// Shows `state` in both controls: the dislike here and the favorite on the star.
    fn correct(&self, item: &CatalogItem, state: RatingState) {
        self.favorite.show_confirmed(item, state.is_favorite());
        self.show_confirmed(item, state.is_disliked());
    }
}

/// The dislike state of one item, as a boolean action. Clones share the state.
#[derive(Clone)]
pub struct DislikeControl {
    inner: Rc<Inner>,
}

impl DislikeControl {
    /// Makes a control with no item. Its action is off. `anchor` is a widget of the view.
    /// `favorite` is the star of the same item: the two controls exclude each other, and `favorite`
    /// can have only one linked dislike control.
    pub fn new(anchor: &impl IsA<gtk::Widget>, favorite: &FavoriteControl) -> Self {
        let action = SimpleAction::new_stateful(ACTION_NAME, None, &false.to_variant());
        action.set_enabled(false);
        let inner = Rc::new(Inner {
            action,
            state: RefCell::default(),
            favorite: favorite.clone(),
            anchor: anchor.as_ref().downgrade(),
            cleared_by_star: Cell::new(false),
            item: RefCell::default(),
            on_change: RefCell::default(),
        });
        let weak: Weak<Inner> = Rc::downgrade(&inner);
        inner.action.connect_change_state(move |_, value| {
            let target = value.and_then(Variant::get::<bool>);
            if let (Some(inner), Some(target)) = (weak.upgrade(), target) {
                inner.toggle(target);
            }
        });
        let weak = Rc::downgrade(&inner);
        favorite.connect_change(move |change| {
            if let Some(inner) = weak.upgrade() {
                inner.follow(change);
            }
        });
        Self { inner }
    }

    /// Calls `on_change` with the dislike state each time it changes. One handler at a time.
    pub fn connect_change(&self, on_change: impl Fn(bool) + 'static) {
        *self.inner.on_change.borrow_mut() = Some(Box::new(on_change));
    }

    /// The action. Add it to the group of a menu, for the "Suggest Less" item.
    pub fn action(&self) -> &SimpleAction {
        &self.inner.action
    }

    /// Shows `item` with an unknown status and no request. Call [`Self::load_status`] when the
    /// menu opens. A result for an earlier item is dropped.
    pub fn set_item(&self, item: Option<CatalogItem>) {
        let mut state = self.inner.state.borrow_mut();
        state.reset(item.clone());
        if let Some(item) = &item
            && let Some(disliked) = remembered(item)
            && let Some((generation, _)) = state.begin_load()
        {
            state.finish_load(generation, disliked);
        }
        drop(state);
        *self.inner.item.borrow_mut() = item;
        self.inner.cleared_by_star.set(false);
        self.inner.sync();
    }

    /// Reads the rating together with the other controls that queue in the same moment. The rows
    /// of a list use this: they get one request for many songs, not one request for each row. A
    /// song that the session changed has no read.
    pub fn queue_status(&self) {
        if let Some((generation, item)) = self.begin_load() {
            StatusBatch::queue(&READS, self.target(), generation, item);
        }
    }

    /// Reads the rating of the item, live. The item stays off until the result comes. A
    /// failure counts as no dislike. An item that the session changed has no read.
    pub fn load_status(&self) {
        if let Some((generation, item)) = self.begin_load() {
            StatusBatch::read_now(&READS, self.target(), generation, item);
        }
    }

    /// Starts a read and shows that the rating loads. `None` if no read is necessary, also
    /// for an item that the session changed.
    fn begin_load(&self) -> Option<(u64, CatalogItem)> {
        let changed_in_session = self
            .inner
            .item
            .borrow()
            .as_ref()
            .is_some_and(|item| remembered(item).is_some());
        if changed_in_session {
            return None;
        }
        let request = self.inner.state.borrow_mut().begin_load();
        if request.is_some() {
            self.inner.sync();
        }
        request
    }

    fn target(&self) -> Weak<dyn StatusTarget> {
        Rc::downgrade(&self.inner) as Weak<dyn StatusTarget>
    }
}

impl StatusTarget for Inner {
    fn is_loading(&self, generation: u64) -> bool {
        self.state.borrow().is_loading(generation)
    }

    fn finish_load(&self, generation: u64, disliked: bool) {
        let current = self.state.borrow_mut().finish_load(generation, disliked);
        if current {
            self.sync();
        }
    }
}

thread_local! {
    /// The rating reads of the dislike controls. A failure counts as "no dislike", like for the
    /// star: the item stays usable, and Apple ignores a duplicate rating.
    static READS: StatusBatch = const { StatusBatch::new(|items| Box::pin(read_dislikes(items))) };
}

/// Reads which of `items` have a dislike, live. After a failure the result is empty.
async fn read_dislikes(items: Vec<CatalogItem>) -> HashMap<CatalogItem, bool> {
    let Some(services) = services::installed() else {
        return HashMap::new();
    };
    let api = Arc::clone(services.api());
    let task = relm4::spawn(async move { write::dislike_status(api.as_ref(), &items).await });
    match task.await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            warn!(%error, "cannot read the ratings");
            HashMap::new()
        }
        Err(join_error) => {
            warn!(%join_error, "the ratings task did not finish");
            HashMap::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATES: [RatingState; 3] = [
        RatingState::Unrated,
        RatingState::Favorite,
        RatingState::Dislike,
    ];

    #[test]
    fn next_request_gives_nothing_for_an_unchanged_state() {
        for state in STATES {
            assert_eq!(next_request(state, state), None);
        }
    }

    #[test]
    fn next_request_sets_a_dislike_from_unrated_and_from_favorite() {
        assert_eq!(
            next_request(RatingState::Unrated, RatingState::Dislike),
            Some(RatingRequest::SetDislike)
        );
        assert_eq!(
            next_request(RatingState::Favorite, RatingState::Dislike),
            Some(RatingRequest::SetDislike)
        );
    }

    #[test]
    fn next_request_removes_the_rating_to_clear_a_dislike_or_a_favorite() {
        assert_eq!(
            next_request(RatingState::Dislike, RatingState::Unrated),
            Some(RatingRequest::RemoveRating)
        );
        assert_eq!(
            next_request(RatingState::Favorite, RatingState::Unrated),
            Some(RatingRequest::RemoveRating)
        );
    }

    #[test]
    fn next_request_adds_a_favorite_from_unrated_and_from_dislike() {
        assert_eq!(
            next_request(RatingState::Unrated, RatingState::Favorite),
            Some(RatingRequest::AddFavorite)
        );
        assert_eq!(
            next_request(RatingState::Dislike, RatingState::Favorite),
            Some(RatingRequest::AddFavorite)
        );
    }
}
