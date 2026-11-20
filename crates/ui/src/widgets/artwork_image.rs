// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Image that shows artwork. It loads the picture in the background.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use relm4::gtk;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use ringo_am_api::artwork_url;
use ringo_core::artwork::ArtworkStore;

use crate::artwork;

/// Icon that the image shows until the artwork is ready.
const PLACEHOLDER_ICON: &str = "audio-x-generic-symbolic";

/// Size of the image before the first request, in logical pixels.
const PLACEHOLDER_SIZE: i32 = 32;

/// A square image for the artwork of a song or an album. A list row can reuse
/// it: a new request replaces the result of an old one.
pub struct ArtworkImage {
    state: Rc<State>,
}

/// The data of an [`ArtworkImage`]. The scale-factor handler holds it weakly.
struct State {
    image: gtk::Image,
    store: ArtworkStore,
    /// Last template and size that the caller set. A new scale factor uses them again.
    wanted: RefCell<(Option<String>, i32)>,
    /// URL that the image shows or loads.
    url: RefCell<Option<String>>,
    /// Number of the last request. A load that ends for an older number is dropped.
    request: Rc<Cell<u64>>,
}

impl ArtworkImage {
    /// Makes an image with the placeholder icon.
    pub fn new(store: ArtworkStore) -> Self {
        let image = gtk::Image::new();
        image.set_overflow(gtk::Overflow::Hidden);
        image.add_css_class("card");
        let state = Rc::new(State {
            image,
            store,
            wanted: RefCell::new((None, PLACEHOLDER_SIZE)),
            url: RefCell::new(None),
            request: Rc::new(Cell::new(0)),
        });
        state.show_placeholder(PLACEHOLDER_SIZE);
        let weak = Rc::downgrade(&state);
        state.image.connect_scale_factor_notify(move |_| {
            if let Some(state) = weak.upgrade() {
                state.refresh();
            }
        });
        Self { state }
    }

    /// The widget to put in a container.
    pub fn widget(&self) -> &gtk::Image {
        &self.state.image
    }

    /// Shows the artwork of `template` at `size` logical pixels. Without a
    /// template, it shows the placeholder. Calling it again with the same
    /// request does nothing. The image loads the artwork again when the scale
    /// factor of the window changes.
    pub fn set_artwork(&self, template: Option<&str>, size: i32) {
        self.state.set_artwork(template, size);
    }
}

impl State {
    /// Repeats the last request. It does nothing if the URL is the same.
    fn refresh(&self) {
        let (template, size) = self.wanted.borrow().clone();
        self.set_artwork(template.as_deref(), size);
    }

    fn set_artwork(&self, template: Option<&str>, size: i32) {
        *self.wanted.borrow_mut() = (template.map(str::to_owned), size);
        self.image.set_size_request(size, size);
        let Some(url) = template.map(|template| self.url_for(template, size)) else {
            self.start_request(None);
            self.show_placeholder(size);
            return;
        };
        if self.url.borrow().as_deref() == Some(url.as_str()) {
            return;
        }
        let request = self.start_request(Some(url.clone()));
        if let Some(texture) = artwork::cached_texture(&url) {
            self.show(&texture, size);
            return;
        }
        self.show_placeholder(size);
        self.load(url, request, size);
    }

    /// The URL for the pixels of the screen.
    fn url_for(&self, template: &str, size: i32) -> String {
        let pixels = u32::try_from(size * self.image.scale_factor()).unwrap_or(1);
        artwork_url(template, pixels, pixels)
    }

    fn start_request(&self, url: Option<String>) -> u64 {
        *self.url.borrow_mut() = url;
        let request = self.request.get() + 1;
        self.request.set(request);
        request
    }

    fn load(&self, url: String, request: u64, size: i32) {
        let store = self.store.clone();
        let latest = Rc::clone(&self.request);
        let image = self.image.downgrade();
        glib::spawn_future_local(async move {
            let texture = artwork::load_texture(&store, url).await;
            if latest.get() != request {
                return;
            }
            if let (Some(texture), Some(image)) = (texture, image.upgrade()) {
                image.remove_css_class("dim-label");
                image.set_pixel_size(size);
                image.set_paintable(Some(&texture));
            }
        });
    }

    fn show(&self, texture: &gtk::gdk::Texture, size: i32) {
        self.image.remove_css_class("dim-label");
        self.image.set_pixel_size(size);
        self.image.set_paintable(Some(texture));
    }

    /// Shows the icon at half of `size`, so it sits inside the rounded square.
    fn show_placeholder(&self, size: i32) {
        self.image.add_css_class("dim-label");
        self.image.set_pixel_size(size / 2);
        self.image.set_icon_name(Some(PLACEHOLDER_ICON));
    }
}
