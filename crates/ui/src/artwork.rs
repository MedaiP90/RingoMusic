// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Textures of artwork. The files come from the disk cache of the core. This
//! module keeps the decoded textures in a bounded memory cache.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use relm4::gtk::gdk::Texture;
use relm4::gtk::gio;
use ringo_core::artwork::ArtworkStore;
use tracing::warn;

/// Number of textures that the memory cache keeps.
const MEMORY_ENTRIES: usize = 200;

/// A map with a fixed capacity. When it is full, an insert removes the entry
/// that was added first.
struct BoundedCache<V> {
    capacity: usize,
    order: VecDeque<String>,
    entries: HashMap<String, V>,
}

impl<V> BoundedCache<V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: String, value: V) {
        if self.entries.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

thread_local! {
    // Textures belong to the main thread, so the cache does too.
    static TEXTURES: RefCell<BoundedCache<Texture>> =
        RefCell::new(BoundedCache::new(MEMORY_ENTRIES));
}

thread_local! {
    // A list item is set up without any context, so its image gets the store here.
    static STORE: RefCell<Option<ArtworkStore>> = const { RefCell::new(None) };
}

/// Sets the store that [`shared_store`] returns. Call it once, at start-up,
/// on the main thread.
pub fn install_store(store: ArtworkStore) {
    STORE.with_borrow_mut(|installed| *installed = Some(store));
}

/// Returns the store from [`install_store`].
///
/// # Panics
///
/// Panics if the store is not installed.
pub fn shared_store() -> ArtworkStore {
    STORE
        .with_borrow(Clone::clone)
        .expect("internal error: the artwork store is installed at start-up")
}

/// Returns the texture of `url` if the memory cache has it.
pub fn cached_texture(url: &str) -> Option<Texture> {
    TEXTURES.with_borrow(|cache| cache.get(url).cloned())
}

/// Returns the texture of `url`. The file comes from the disk cache or from the
/// network, and the decoding runs on a blocking thread. The call returns `None`
/// if the artwork is not available.
///
/// Call it on the main thread.
pub async fn load_texture(store: &ArtworkStore, url: String) -> Option<Texture> {
    if let Some(texture) = cached_texture(&url) {
        return Some(texture);
    }
    let download = {
        let (store, url) = (store.clone(), url.clone());
        relm4::spawn(async move { store.fetch(&url).await })
    };
    let path = download.await.ok()??;
    let decoded = gio::spawn_blocking(move || Texture::from_filename(&path)).await;
    let texture = match decoded {
        Ok(Ok(texture)) => texture,
        Ok(Err(error)) => {
            warn!(%error, url, "cannot decode artwork");
            return None;
        }
        Err(_) => {
            warn!(url, "the artwork decoder stopped");
            return None;
        }
    };
    TEXTURES.with_borrow_mut(|cache| cache.insert(url, texture.clone()));
    Some(texture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_removes_the_oldest_entry_when_full() {
        let mut cache = BoundedCache::new(2);
        cache.insert("a".to_owned(), 1);
        cache.insert("b".to_owned(), 2);
        cache.insert("c".to_owned(), 3);
        assert_eq!(
            (cache.get("a"), cache.get("b"), cache.get("c")),
            (None, Some(&2), Some(&3))
        );
    }

    #[test]
    fn insert_of_a_known_key_replaces_the_value_without_eviction() {
        let mut cache = BoundedCache::new(2);
        cache.insert("a".to_owned(), 1);
        cache.insert("b".to_owned(), 2);
        cache.insert("a".to_owned(), 3);
        assert_eq!((cache.get("a"), cache.get("b")), (Some(&3), Some(&2)));
    }
}
