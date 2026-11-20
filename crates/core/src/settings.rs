// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Typed access to the user settings (`GSettings`).

use gio::prelude::*;

use crate::{Error, Result};

/// Schema ID of the application. It is the same as the application ID.
pub const SCHEMA_ID: &str = "io.github.MedaiP90.RingoMusic";

const WINDOW_WIDTH: &str = "window-width";
const WINDOW_HEIGHT: &str = "window-height";
const WINDOW_MAXIMIZED: &str = "window-maximized";
const VOLUME: &str = "volume";
const QUIT_ON_CLOSE: &str = "quit-on-close";
const SHOW_NOTIFICATIONS: &str = "show-notifications";
const BACKGROUND_DENIED: &str = "background-denied";
const CHECK_UPDATES: &str = "check-updates";
const LAST_UPDATE_CHECK: &str = "last-update-check";
const API_CACHE_MAX_MB: &str = "api-cache-max-mb";

/// Typed view of the application settings.
///
/// Each setter returns an error when the value is outside the range of the key.
#[derive(Clone, Debug)]
pub struct Settings {
    inner: gio::Settings,
}

impl Settings {
    /// Opens the settings with the default backend.
    ///
    /// # Panics
    ///
    /// Panics if the schema is not installed. For development, set
    /// `GSETTINGS_SCHEMA_DIR` to a directory with the compiled schema.
    #[must_use]
    pub fn new() -> Self {
        Self::from_raw(gio::Settings::new(SCHEMA_ID))
    }

    /// Wraps an existing `gio::Settings` of the application schema.
    #[must_use]
    pub fn from_raw(inner: gio::Settings) -> Self {
        Self { inner }
    }

    /// Returns the raw object. Use it to bind widgets to keys.
    #[must_use]
    pub fn raw(&self) -> &gio::Settings {
        &self.inner
    }

    /// Width of the main window, in pixels.
    #[must_use]
    pub fn window_width(&self) -> i32 {
        self.inner.int(WINDOW_WIDTH)
    }

    /// Sets the width of the main window.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_window_width(&self, value: i32) -> Result<()> {
        Self::write(WINDOW_WIDTH, self.inner.set_int(WINDOW_WIDTH, value))
    }

    /// Height of the main window, in pixels.
    #[must_use]
    pub fn window_height(&self) -> i32 {
        self.inner.int(WINDOW_HEIGHT)
    }

    /// Sets the height of the main window.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_window_height(&self, value: i32) -> Result<()> {
        Self::write(WINDOW_HEIGHT, self.inner.set_int(WINDOW_HEIGHT, value))
    }

    /// Whether the main window was maximized.
    #[must_use]
    pub fn window_maximized(&self) -> bool {
        self.inner.boolean(WINDOW_MAXIMIZED)
    }

    /// Sets whether the main window is maximized.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_window_maximized(&self, value: bool) -> Result<()> {
        Self::write(
            WINDOW_MAXIMIZED,
            self.inner.set_boolean(WINDOW_MAXIMIZED, value),
        )
    }

    /// Volume of the last session, from 0.0 to 1.0.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.inner.double(VOLUME)
    }

    /// Sets the volume.
    ///
    /// # Errors
    ///
    /// Returns an error if `value` is not between 0.0 and 1.0.
    pub fn set_volume(&self, value: f64) -> Result<()> {
        Self::write(VOLUME, self.inner.set_double(VOLUME, value))
    }

    /// Whether closing the window stops the application.
    #[must_use]
    pub fn quit_on_close(&self) -> bool {
        self.inner.boolean(QUIT_ON_CLOSE)
    }

    /// Sets whether closing the window stops the application.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_quit_on_close(&self, value: bool) -> Result<()> {
        Self::write(QUIT_ON_CLOSE, self.inner.set_boolean(QUIT_ON_CLOSE, value))
    }

    /// Whether the application shows track notifications.
    #[must_use]
    pub fn show_notifications(&self) -> bool {
        self.inner.boolean(SHOW_NOTIFICATIONS)
    }

    /// Sets whether the application shows track notifications.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_show_notifications(&self, value: bool) -> Result<()> {
        Self::write(
            SHOW_NOTIFICATIONS,
            self.inner.set_boolean(SHOW_NOTIFICATIONS, value),
        )
    }

    /// Whether the background portal refused the permission and the user was
    /// not told yet.
    #[must_use]
    pub fn background_denied(&self) -> bool {
        self.inner.boolean(BACKGROUND_DENIED)
    }

    /// Sets whether the background portal refused the permission.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_background_denied(&self, value: bool) -> Result<()> {
        Self::write(
            BACKGROUND_DENIED,
            self.inner.set_boolean(BACKGROUND_DENIED, value),
        )
    }

    /// Whether the application checks for updates.
    #[must_use]
    pub fn check_updates(&self) -> bool {
        self.inner.boolean(CHECK_UPDATES)
    }

    /// Sets whether the application checks for updates.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_check_updates(&self, value: bool) -> Result<()> {
        Self::write(CHECK_UPDATES, self.inner.set_boolean(CHECK_UPDATES, value))
    }

    /// Unix time, in seconds, of the last update check. Zero means never.
    #[must_use]
    pub fn last_update_check(&self) -> i64 {
        self.inner.int64(LAST_UPDATE_CHECK)
    }

    /// Sets the Unix time of the last update check.
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be written.
    pub fn set_last_update_check(&self, value: i64) -> Result<()> {
        Self::write(
            LAST_UPDATE_CHECK,
            self.inner.set_int64(LAST_UPDATE_CHECK, value),
        )
    }

    /// Size limit of the API cache, in mebibytes.
    #[must_use]
    pub fn api_cache_max_mb(&self) -> u32 {
        self.inner.uint(API_CACHE_MAX_MB)
    }

    /// Sets the size limit of the API cache.
    ///
    /// # Errors
    ///
    /// Returns an error if `value` is not between 16 and 4096.
    pub fn set_api_cache_max_mb(&self, value: u32) -> Result<()> {
        Self::write(
            API_CACHE_MAX_MB,
            self.inner.set_uint(API_CACHE_MAX_MB, value),
        )
    }

    /// Adds the key name to a failed write.
    fn write(
        key: &'static str,
        result: std::result::Result<(), gio::glib::BoolError>,
    ) -> Result<()> {
        result.map_err(|source| Error::WriteSetting { key, source })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
