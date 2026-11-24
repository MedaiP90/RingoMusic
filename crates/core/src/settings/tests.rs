// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use gio::SettingsSchemaSource;

use super::*;
use crate::api_cache::DEFAULT_MAX_BYTES;

/// Opens the compiled schema on a memory backend. No system install is needed.
fn test_settings() -> Settings {
    let source = SettingsSchemaSource::from_directory(env!("RINGO_SCHEMA_DIR"), None, false)
        .expect("build.rs compiled the schema");
    let schema = source
        .lookup(SCHEMA_ID, false)
        .expect("schema is in the compiled directory");
    let backend = gio::memory_settings_backend_new();
    Settings::from_raw(gio::Settings::new_full(&schema, Some(&backend), None))
}

#[test]
fn defaults_match_the_schema_table() {
    let settings = test_settings();
    assert_eq!(settings.window_width(), 1000);
    assert_eq!(settings.window_height(), 700);
    assert!(!settings.window_maximized());
    assert!((settings.volume() - 1.0).abs() < f64::EPSILON);
    assert!(!settings.quit_on_close());
    assert!(settings.show_notifications());
    assert!(!settings.background_denied());
    assert!(settings.check_updates());
    assert_eq!(settings.last_update_check(), 0);
    assert_eq!(settings.api_cache_max_mb(), 50);
    assert_eq!(settings.artwork_cache_max_mb(), 500);
    assert_eq!(settings.accent_color(), "system");
    assert_eq!(settings.color_scheme(), "system");
}

#[test]
fn setters_store_values() {
    let settings = test_settings();
    settings.set_window_width(800).expect("value is in range");
    settings
        .set_window_maximized(true)
        .expect("value is in range");
    settings
        .set_last_update_check(1_700_000_000)
        .expect("value is in range");
    assert_eq!(settings.window_width(), 800);
    assert!(settings.window_maximized());
    assert_eq!(settings.last_update_check(), 1_700_000_000);
}

#[test]
fn background_denied_flag_can_be_set_and_cleared() {
    let settings = test_settings();
    settings
        .set_background_denied(true)
        .expect("key is writable");
    assert!(settings.background_denied());
    settings
        .set_background_denied(false)
        .expect("key is writable");
    assert!(!settings.background_denied());
}

#[test]
fn volume_accepts_the_range_limits() {
    let settings = test_settings();
    settings.set_volume(0.0).expect("value is in range");
    assert!(settings.volume().abs() < f64::EPSILON);
    settings.set_volume(1.0).expect("value is in range");
}

#[test]
fn volume_rejects_values_outside_the_range() {
    let settings = test_settings();
    assert!(matches!(
        settings.set_volume(-0.1),
        Err(Error::WriteSetting { key: "volume", .. })
    ));
    assert!(settings.set_volume(1.1).is_err());
    assert!((settings.volume() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn color_scheme_accepts_only_the_choices() {
    let settings = test_settings();
    assert!(settings.set_color_scheme("sepia").is_err());
    settings.set_color_scheme("dark").expect("dark is a choice");
    assert_eq!(settings.color_scheme(), "dark");
}

#[test]
fn accent_color_accepts_only_the_choices() {
    let settings = test_settings();
    assert!(settings.set_accent_color("magenta").is_err());
    settings.set_accent_color("teal").expect("teal is a choice");
    assert_eq!(settings.accent_color(), "teal");
}

#[test]
fn artwork_cache_size_rejects_values_outside_the_range() {
    let settings = test_settings();
    assert!(settings.raw().set_uint("artwork-cache-max-mb", 15).is_err());
    assert!(
        settings
            .raw()
            .set_uint("artwork-cache-max-mb", 4097)
            .is_err()
    );
    settings
        .raw()
        .set_uint("artwork-cache-max-mb", 4096)
        .expect("value is in range");
    assert_eq!(settings.artwork_cache_max_mb(), 4096);
}

#[test]
fn api_cache_size_accepts_the_range_limits() {
    let settings = test_settings();
    settings
        .raw()
        .set_uint("api-cache-max-mb", 16)
        .expect("value is in range");
    settings
        .raw()
        .set_uint("api-cache-max-mb", 4096)
        .expect("value is in range");
    assert_eq!(settings.api_cache_max_mb(), 4096);
}

#[test]
fn api_cache_size_rejects_values_outside_the_range() {
    let settings = test_settings();
    assert!(settings.raw().set_uint("api-cache-max-mb", 15).is_err());
    assert!(settings.raw().set_uint("api-cache-max-mb", 4097).is_err());
    assert_eq!(settings.api_cache_max_mb(), 50);
}

#[test]
fn api_cache_default_equals_the_cache_constant() {
    let default_bytes = u64::from(test_settings().api_cache_max_mb()) * 1024 * 1024;
    assert_eq!(default_bytes, DEFAULT_MAX_BYTES);
}
