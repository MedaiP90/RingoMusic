// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Tints from the dominant color of a cover. The player bar and the queue panel
//! use the playing song (`--cover-tint`). An album page that is on screen uses
//! its own album (`--album-tint`). `style.css` uses both variables.

use std::cell::RefCell;

use relm4::gtk;
use relm4::gtk::gdk;
use relm4::gtk::prelude::*;

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

/// The colors that the window shows.
struct Tint {
    provider: gtk::CssProvider,
    playing: Option<Rgb>,
    /// The album page on screen: its ID and its color.
    viewed: Option<(String, Rgb)>,
}

thread_local! {
    // The tint belongs to the main thread, like the widgets that it colors.
    static TINT: RefCell<Option<Tint>> = const { RefCell::new(None) };
}

/// Follows the cover that `image` shows. The playing song sets the cover tint.
pub fn follow_playing(image: &gtk::Image) {
    let update = |image: &gtk::Image| update_tint(|tint| tint.playing = color_of(image));
    image.connect_paintable_notify(update);
    image.connect_icon_name_notify(update);
}

/// Follows the cover that `image` shows for the page `owner`. While `page` is
/// on screen, the album tint is the color of the cover.
pub fn follow_viewed(image: &gtk::Image, page: &impl IsA<gtk::Widget>, owner: &str) {
    let on_screen = |page: &gtk::Widget| page.is_mapped();
    let show = {
        let (image, page, owner) = (image.clone(), page.clone().upcast(), owner.to_owned());
        move || {
            let color = color_of(&image).filter(|_| on_screen(&page));
            update_tint(|tint| tint.set_viewed(&owner, color));
        }
    };
    let (on_cover, on_map, on_unmap) = (show.clone(), show.clone(), show);
    image.connect_paintable_notify(move |_| on_cover());
    page.connect_map(move |_| on_map());
    page.connect_unmap(move |_| on_unmap());
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
            playing: None,
            viewed: None,
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

    fn apply(&self) {
        let viewed = self.viewed.as_ref().map(|(_, color)| *color);
        let declarations: String = [("cover-tint", self.playing), ("album-tint", viewed)]
            .into_iter()
            .filter_map(|(name, color)| {
                color.map(|Rgb { r, g, b }| format!("--{name}: rgb({r}, {g}, {b}); "))
            })
            .collect();
        self.provider
            .load_from_string(&format!("window {{ {declarations}}}"));
    }
}

/// Changes the tint with `change`, then shows it.
fn update_tint(change: impl FnOnce(&mut Tint)) {
    TINT.with_borrow_mut(|slot| {
        let tint = slot.get_or_insert_with(Tint::new);
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
