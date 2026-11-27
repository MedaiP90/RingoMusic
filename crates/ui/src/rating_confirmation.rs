// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Confirms a rating write with Apple, so the star and "Suggest Less" show the real state
//! (decision L20, changed by the user on 2026-11-27).
//!
//! Apple can answer with the old state for a short time after a write (`202` means "accepted").
//! So the state is read again at [`READ_TIMES`] after the write, until it matches the expected
//! state. If no read matches, the last read wins and a toast tells the user. A newer write of the
//! same item cancels the older confirmation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Weak;
use std::slice;
use std::sync::Arc;
use std::time::Duration;

use gettextrs::gettext;
use relm4::gtk;
use relm4::gtk::glib;
use ringo_am_api::write::{self, CatalogItem};
use tracing::warn;

use crate::add_to_playlist;
use crate::detail;
use crate::library_write;

/// Times after the write when the state is read again.
const READ_TIMES: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// The rating state of an item. A favorite and a dislike never hold together.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum RatingState {
    Unrated,
    Favorite,
    Dislike,
}

impl RatingState {
    /// Makes the state from the two values that Apple returns. A favorite wins, because Apple
    /// clears the dislike when it adds a favorite.
    fn from_status(favorite: bool, disliked: bool) -> Self {
        match (favorite, disliked) {
            (true, _) => Self::Favorite,
            (false, true) => Self::Dislike,
            (false, false) => Self::Unrated,
        }
    }

    pub(crate) fn is_favorite(self) -> bool {
        self == Self::Favorite
    }

    pub(crate) fn is_disliked(self) -> bool {
        self == Self::Dislike
    }
}

/// The result of all the reads of one confirmation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Outcome {
    /// Apple has the expected state.
    Confirmed,
    /// The last successful read gave this other state.
    Differs(RatingState),
    /// No read was successful.
    Unknown,
}

/// Gives the outcome of `reads` (`None` is a failed read). The last successful read decides.
fn outcome(expected: RatingState, reads: &[Option<RatingState>]) -> Outcome {
    match reads.iter().rev().find_map(|read| *read) {
        Some(state) if state == expected => Outcome::Confirmed,
        Some(state) => Outcome::Differs(state),
        None => Outcome::Unknown,
    }
}

thread_local! {
    /// The token of the latest write of each item.
    static LATEST: RefCell<HashMap<CatalogItem, u64>> = RefCell::new(HashMap::new());
    static NEXT_TOKEN: Cell<u64> = const { Cell::new(0) };
}

/// One rating write of one item. Make it when the user clicks, before the request: then a newer
/// click cancels the confirmation of an older write, also while the newer request runs.
pub(crate) struct WriteToken {
    item: CatalogItem,
    id: u64,
}

impl WriteToken {
    pub(crate) fn begin(item: &CatalogItem) -> Self {
        let id = NEXT_TOKEN.replace(NEXT_TOKEN.get() + 1);
        LATEST.with_borrow_mut(|latest| latest.insert(item.clone(), id));
        Self {
            item: item.clone(),
            id,
        }
    }

    pub(crate) fn item(&self) -> &CatalogItem {
        &self.item
    }

    fn is_latest(&self) -> bool {
        LATEST.with_borrow(|latest| latest.get(&self.item) == Some(&self.id))
    }
}

/// A control that shows a rating state and takes the state of Apple after a write.
pub(crate) trait RatingControl {
    /// A widget of the view, if it still exists. The toast appears over its window.
    fn anchor(&self) -> Option<gtk::Widget>;

    /// Shows `state`, the state that Apple has for `item` after a write.
    fn correct(&self, item: &CatalogItem, state: RatingState);
}

/// Reads the state after the write of `token` until it is `expected`. If Apple has another
/// state, `control` shows it and a toast tells the user. Does nothing if a newer write of the
/// same item started. A weak `control` lets a closed view go away while the reads wait.
pub(crate) fn confirm<C: RatingControl + 'static>(
    control: &Weak<C>,
    token: WriteToken,
    expected: RatingState,
) {
    let control = control.clone();
    glib::spawn_future_local(async move {
        let Some(outcome) = read_until_confirmed(&token, expected).await else {
            return;
        };
        match outcome {
            Outcome::Confirmed => {}
            Outcome::Differs(state) => {
                warn!(
                    ?expected,
                    ?state,
                    "Apple has another rating state than expected"
                );
                if let Some(control) = control.upgrade() {
                    show_correction(control.as_ref(), token.item(), state);
                }
            }
            Outcome::Unknown => warn!("cannot read the rating state to confirm a write"),
        }
    });
}

fn show_correction(control: &impl RatingControl, item: &CatalogItem, state: RatingState) {
    control.correct(item, state);
    if let Some(anchor) = control.anchor() {
        library_write::notify(&anchor, &gettext("Apple Music did not confirm the change"));
    }
}

/// Reads the state at each of [`READ_TIMES`] and stops at the first match. Returns `None` if a
/// newer write of the same item started.
async fn read_until_confirmed(token: &WriteToken, expected: RatingState) -> Option<Outcome> {
    let mut reads = Vec::with_capacity(READ_TIMES.len());
    let mut elapsed = Duration::ZERO;
    for read_time in READ_TIMES {
        glib::timeout_future(read_time.saturating_sub(elapsed)).await;
        elapsed = read_time;
        if !token.is_latest() {
            return None;
        }
        let read = read_state(token.item.clone()).await;
        if !token.is_latest() {
            return None;
        }
        reads.push(read);
        if read == Some(expected) {
            break;
        }
    }
    Some(outcome(expected, &reads))
}

/// Reads the favorite and the dislike of `item`, live. Returns `None` after a failure.
async fn read_state(item: CatalogItem) -> Option<RatingState> {
    let services = add_to_playlist::services()?;
    let storefront = detail::storefront(&services.core().subscribe().borrow()).unwrap_or_default();
    let api = Arc::clone(services.api());
    let task = relm4::spawn(async move {
        let items = slice::from_ref(&item);
        let favorite_status = write::favorite_status(api.as_ref(), &storefront, items).await?;
        // Gives an empty result with no request for an item that is not a song.
        let dislike_status = write::dislike_status(api.as_ref(), items).await?;
        let favorite = favorite_status.get(&item).copied().unwrap_or(false);
        let disliked = dislike_status.get(&item).copied().unwrap_or(false);
        Ok::<_, ringo_am_api::Error>(RatingState::from_status(favorite, disliked))
    });
    match task.await {
        Ok(Ok(state)) => Some(state),
        Ok(Err(error)) => {
            warn!(%error, "cannot read the rating state");
            None
        }
        Err(join_error) => {
            warn!(%join_error, "the rating state task did not finish");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str) -> CatalogItem {
        CatalogItem::Song(id.to_owned().into())
    }

    #[test]
    fn outcome_is_confirmed_when_the_last_read_matches() {
        let reads = [Some(RatingState::Unrated), Some(RatingState::Favorite)];
        assert_eq!(outcome(RatingState::Favorite, &reads), Outcome::Confirmed);
    }

    #[test]
    fn outcome_gives_the_last_successful_read_when_no_read_matches() {
        let reads = [Some(RatingState::Unrated), Some(RatingState::Dislike), None];
        assert_eq!(
            outcome(RatingState::Favorite, &reads),
            Outcome::Differs(RatingState::Dislike)
        );
    }

    #[test]
    fn outcome_is_unknown_when_every_read_failed() {
        assert_eq!(
            outcome(RatingState::Favorite, &[None, None, None]),
            Outcome::Unknown
        );
    }

    #[test]
    fn a_favorite_wins_over_a_dislike() {
        assert_eq!(RatingState::from_status(true, true), RatingState::Favorite);
        assert_eq!(RatingState::from_status(false, true), RatingState::Dislike);
        assert_eq!(RatingState::from_status(false, false), RatingState::Unrated);
    }

    #[test]
    fn a_newer_write_of_the_same_item_cancels_the_older_one() {
        let older = WriteToken::begin(&song("1"));
        let other_item = WriteToken::begin(&song("2"));
        assert!(older.is_latest());
        let newer = WriteToken::begin(&song("1"));
        assert!(!older.is_latest());
        assert!(newer.is_latest());
        assert!(other_item.is_latest());
    }
}
