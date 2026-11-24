// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tints from the dominant color of a cover. The player bar and the queue panel
//! use the playing song (`--cover-tint`). An album or artist page that is on
//! screen uses its own cover or photo (`--album-tint`). `style.css` uses both
//! variables.
//!
//! GTK runs no CSS transition when a style provider changes, but it runs one
//! when a class changes. So the provider keeps two slots of each variable,
//! and a class on the window selects one. A new color goes into the free slot,
//! and then the window changes its class. The `transition` rules in
//! `style.css` then fade the color.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::gdk;
use relm4::gtk::glib;

/// Samples that the color search reads on each side of the cover.
const SAMPLES_PER_SIDE: usize = 24;

/// Weight of a grey pixel, compared with a pixel of full chroma (1.0).
const GREY_WEIGHT: f64 = 0.01;

/// Pixels with less alpha than this do not count.
const MIN_ALPHA: u8 = 128;

/// An opaque color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

/// One tint variable, with two slots. The window shows the active slot.
struct Variable {
    name: &'static str,
    slots: [Option<Rgb>; 2],
    active: usize,
}

impl Variable {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            slots: [None; 2],
            active: 0,
        }
    }

    /// Puts `color` in the free slot and makes that slot active. Nothing
    /// changes if the active slot already has `color`.
    fn set(&mut self, color: Option<Rgb>) {
        if self.slots[self.active] != color {
            self.active = 1 - self.active;
            self.slots[self.active] = color;
        }
    }

    /// The window class that selects `slot`.
    fn class(&self, slot: usize) -> String {
        format!("{}-{slot}", self.name)
    }

    /// The CSS rules of the slots that have a color.
    fn css(&self) -> String {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot, color)| {
                color.map(|Rgb { r, g, b }| {
                    let (class, name) = (self.class(slot), self.name);
                    format!("window.{class} {{ --{name}: rgb({r}, {g}, {b}); }} ")
                })
            })
            .collect()
    }

    /// Gives `window` the class of the active slot.
    fn select_on(&self, window: &gtk::Widget) {
        window.remove_css_class(&self.class(1 - self.active));
        window.add_css_class(&self.class(self.active));
    }
}

/// The colors that the window shows.
struct Tint {
    provider: gtk::CssProvider,
    /// The window that gets the slot classes.
    window: glib::WeakRef<gtk::Widget>,
    playing: Option<Rgb>,
    /// The album page on screen: its ID and its color.
    viewed: Option<(String, Rgb)>,
    cover: Variable,
    album: Variable,
    /// The CSS that the provider has now. A change of the provider restyles
    /// the whole window, so the same CSS is not loaded again.
    loaded: String,
}

thread_local! {
    // The tint belongs to the main thread, like the widgets that it colors.
    static TINT: RefCell<Option<Tint>> = const { RefCell::new(None) };
}

/// Follows the cover that `image` shows. The playing song sets the cover tint.
pub fn follow_playing(image: &gtk::Image) {
    let update = |image: &gtk::Image| update_tint(image, |tint| tint.playing = color_of(image));
    image.connect_paintable_notify(update);
    image.connect_icon_name_notify(update);
}

/// Follows the cover that `image` shows for the page `owner`. While `page` is
/// on screen, the album tint is the color of the cover.
///
/// The tint changes only at the end of a navigation transition. A change
/// restyles the whole window, and during the slide that drops frames.
pub fn follow_viewed(image: &gtk::Image, page: &adw::NavigationPage, owner: &str) {
    let shown = Rc::new(Cell::new(false));
    let show = Rc::new({
        let (shown, owner) = (Rc::clone(&shown), owner.to_owned());
        move |image: &gtk::Image| {
            let color = color_of(image).filter(|_| shown.get());
            update_tint(image, |tint| tint.set_viewed(&owner, color));
        }
    });
    image.connect_paintable_notify(glib::clone!(
        #[strong]
        show,
        move |image| show(image)
    ));
    page.connect_shown(glib::clone!(
        #[weak]
        image,
        #[strong]
        shown,
        #[strong]
        show,
        move |_| {
            shown.set(true);
            show(&image);
        }
    ));
    page.connect_hidden(glib::clone!(
        #[weak]
        image,
        move |_| {
            shown.set(false);
            show(&image);
        }
    ));
}

impl Tint {
    fn new() -> Self {
        let provider = gtk::CssProvider::new();
        #[expect(deprecated, reason = "GTK has no other way to add a global style")]
        gtk::StyleContext::add_provider_for_display(
            &gdk::Display::default().expect("internal error: GTK has a display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        Self {
            provider,
            window: glib::WeakRef::new(),
            playing: None,
            viewed: None,
            cover: Variable::new("cover-tint"),
            album: Variable::new("album-tint"),
            loaded: String::new(),
        }
    }

    /// Sets the color of `owner`. `None` removes it, but only if `owner` set it.
    fn set_viewed(&mut self, owner: &str, color: Option<Rgb>) {
        match color {
            Some(color) => self.viewed = Some((owner.to_owned(), color)),
            None if self.viewed.as_ref().is_some_and(|(id, _)| id == owner) => self.viewed = None,
            None => {}
        }
    }

    /// Loads the new colors into the free slots, then selects them on the
    /// window. The class change runs the transition.
    fn apply(&mut self) {
        self.cover.set(self.playing);
        self.album
            .set(self.viewed.as_ref().map(|(_, color)| *color));
        let css = self.cover.css() + &self.album.css();
        if css != self.loaded {
            self.provider.load_from_string(&css);
            self.loaded = css;
        }
        if let Some(window) = self.window.upgrade() {
            self.cover.select_on(&window);
            self.album.select_on(&window);
        }
    }
}

/// Changes the tint with `change`, then shows it on the window of `image`.
fn update_tint(image: &gtk::Image, change: impl FnOnce(&mut Tint)) {
    TINT.with_borrow_mut(|slot| {
        let tint = slot.get_or_insert_with(Tint::new);
        if let Some(root) = image.root() {
            tint.window.set(Some(root.upcast_ref()));
        }
        change(tint);
        tint.apply();
    });
}

/// The dominant color of the cover in `image`. `None` if it shows no texture.
fn color_of(image: &gtk::Image) -> Option<Rgb> {
    let texture = image.paintable()?.downcast::<gdk::Texture>().ok()?;
    let mut downloader = gdk::TextureDownloader::new(&texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let width = usize::try_from(texture.width()).ok()?;
    let height = usize::try_from(texture.height()).ok()?;
    dominant_color(&bytes, width, height, stride)
}

/// Finds the dominant color of RGBA pixels. It reads a grid of pixels and
/// averages them. A vivid pixel weighs more than a grey one, so a cover with
/// a small colored part does not give a dull grey.
fn dominant_color(rgba: &[u8], width: usize, height: usize, stride: usize) -> Option<Rgb> {
    let mut sum = [0.0_f64; 3];
    let mut total_weight = 0.0;
    for row in 0..SAMPLES_PER_SIDE.min(height) {
        for column in 0..SAMPLES_PER_SIDE.min(width) {
            let pixel_row = row * height / SAMPLES_PER_SIDE.min(height);
            let pixel_column = column * width / SAMPLES_PER_SIDE.min(width);
            let offset = pixel_row * stride + pixel_column * 4;
            let Some(&[r, g, b, alpha]) = rgba.get(offset..offset + 4) else {
                continue;
            };
            if alpha < MIN_ALPHA {
                continue;
            }
            let chroma = f64::from(r.max(g).max(b) - r.min(g).min(b)) / 255.0;
            let weight = chroma * chroma + GREY_WEIGHT;
            for (total, channel) in sum.iter_mut().zip([r, g, b]) {
                *total += weight * f64::from(channel);
            }
            total_weight += weight;
        }
    }
    if total_weight == 0.0 {
        return None;
    }
    let [r, g, b] = sum.map(|total| {
        // The mean of u8 values fits in u8.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the mean is between 0 and 255"
        )]
        let channel = (total / total_weight).round() as u8;
        channel
    });
    Some(Rgb { r, g, b })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(pixel: [u8; 4], count: usize) -> Vec<u8> {
        pixel.repeat(count)
    }

    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
    const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };

    #[test]
    fn variable_set_keeps_the_old_color_in_the_other_slot() {
        let mut variable = Variable::new("album-tint");
        variable.set(Some(RED));
        variable.set(Some(BLUE));
        assert_eq!(
            variable.css(),
            "window.album-tint-0 { --album-tint: rgb(0, 0, 255); } \
             window.album-tint-1 { --album-tint: rgb(255, 0, 0); } "
        );
        assert_eq!(variable.class(variable.active), "album-tint-0");
    }

    #[test]
    fn variable_set_with_the_same_color_keeps_the_active_slot() {
        let mut variable = Variable::new("album-tint");
        variable.set(Some(RED));
        let active = variable.active;
        variable.set(Some(RED));
        assert_eq!(variable.active, active);
    }

    #[test]
    fn dominant_color_of_a_solid_image_is_that_color() {
        let image = solid([200, 40, 40, 255], 16);
        assert_eq!(
            dominant_color(&image, 4, 4, 16),
            Some(Rgb {
                r: 200,
                g: 40,
                b: 40
            })
        );
    }

    #[test]
    fn dominant_color_prefers_vivid_pixels_to_grey_ones() {
        let mut image = solid([128, 128, 128, 255], 8);
        image.extend(solid([255, 0, 0, 255], 8));
        let color = dominant_color(&image, 4, 4, 16).expect("the image has pixels");
        assert!(color.r > 200 && color.g < 60, "{color:?} is not red");
    }

    #[test]
    fn dominant_color_ignores_transparent_pixels() {
        assert_eq!(dominant_color(&solid([9, 9, 9, 0], 4), 2, 2, 8), None);
    }
}
