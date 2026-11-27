// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Live reads of a yes/no status of items, for example the favorite star and the dislike.
//!
//! The rows of a list queue their reads. After a short wait, one request reads the status of up
//! to [`BATCH_SIZE`] items, not one request for each row.

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Weak;
use std::thread::LocalKey;
use std::time::Duration;

use relm4::gtk::glib;
use ringo_am_api::write::CatalogItem;

/// Time that the rows of a list wait for each other before one request reads their status.
const BATCH_DELAY: Duration = Duration::from_millis(60);

/// Most items in one status request.
const BATCH_SIZE: usize = 50;

/// The status of each item that a read got. After a failure, the map is empty.
pub type Statuses = HashMap<CatalogItem, bool>;

/// A read of the status of some items.
pub type ReadFuture = Pin<Box<dyn Future<Output = Statuses>>>;

/// A control that shows a status that [`StatusBatch`] reads.
pub trait StatusTarget {
    /// Tells if the control still waits for the read `generation`.
    fn is_loading(&self, generation: u64) -> bool;

    /// Shows `value`, the result of the read `generation`. The control drops the result of an
    /// older read.
    fn finish_load(&self, generation: u64, value: bool);
}

/// A control that waits for a batched read.
struct Waiting {
    target: Weak<dyn StatusTarget>,
    generation: u64,
    item: CatalogItem,
}

impl Waiting {
    /// Tells if the control still waits for this read.
    fn is_current(&self) -> bool {
        self.target
            .upgrade()
            .is_some_and(|target| target.is_loading(self.generation))
    }

    /// Gives the status of the item in `status` to the control. A missing item counts as "no".
    fn finish(&self, status: &Statuses) {
        if let Some(target) = self.target.upgrade() {
            let value = status.get(&self.item).copied().unwrap_or(false);
            target.finish_load(self.generation, value);
        }
    }
}

/// The reads of one status that wait for a request. Keep one in a `thread_local!` for each
/// status.
pub struct StatusBatch {
    waiting: RefCell<Vec<Waiting>>,
    read: fn(Vec<CatalogItem>) -> ReadFuture,
}

impl StatusBatch {
    /// Makes an empty batch. `read` reads the status of some items, live.
    pub const fn new(read: fn(Vec<CatalogItem>) -> ReadFuture) -> Self {
        Self {
            waiting: RefCell::new(Vec::new()),
            read,
        }
    }

    /// Reads the status of `item` for `target`, together with the other reads that queue in the
    /// same moment.
    pub fn queue(
        batch: &'static LocalKey<Self>,
        target: Weak<dyn StatusTarget>,
        generation: u64,
        item: CatalogItem,
    ) {
        let first = batch.with(|batch| {
            let mut waiting = batch.waiting.borrow_mut();
            waiting.push(Waiting {
                target,
                generation,
                item,
            });
            waiting.len() == 1
        });
        if first {
            glib::timeout_add_local_once(BATCH_DELAY, move || batch.with(Self::flush));
        }
    }

    /// Reads the status of `item` for `target` now, in its own request.
    pub fn read_now(
        batch: &'static LocalKey<Self>,
        target: Weak<dyn StatusTarget>,
        generation: u64,
        item: CatalogItem,
    ) {
        let read = batch.with(|batch| (batch.read)(vec![item.clone()]));
        let waiting = Waiting {
            target,
            generation,
            item,
        };
        glib::spawn_future_local(async move { waiting.finish(&read.await) });
    }

    /// Sends the queued reads, in requests of up to [`BATCH_SIZE`] items. A control that got
    /// another item in the meantime does not count.
    fn flush(&self) {
        let mut waiting: Vec<Waiting> = self
            .waiting
            .take()
            .into_iter()
            .filter(Waiting::is_current)
            .collect();
        let mut items: Vec<CatalogItem> = Vec::new();
        for entry in &waiting {
            if !items.contains(&entry.item) {
                items.push(entry.item.clone());
            }
        }
        for chunk in items.chunks(BATCH_SIZE) {
            let entries: Vec<Waiting> = waiting
                .extract_if(.., |entry| chunk.contains(&entry.item))
                .collect();
            let read = (self.read)(chunk.to_vec());
            glib::spawn_future_local(async move {
                let status = read.await;
                for entry in &entries {
                    entry.finish(&status);
                }
            });
        }
    }
}
