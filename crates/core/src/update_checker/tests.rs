// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fs;
use std::path::PathBuf;

use super::*;

/// Shape of a response of `releases/latest`, cut to the useful fields.
const RECORDED_RESPONSE: &str = r#"{
  "url": "https://api.github.com/repos/MedaiP90/RingoMusic/releases/1",
  "html_url": "https://github.com/MedaiP90/RingoMusic/releases/tag/v1.2.3",
  "id": 1,
  "tag_name": "v1.2.3",
  "name": "RingoMusic 1.2.3",
  "draft": false,
  "prerelease": false,
  "assets": []
}"#;

const NOW: i64 = 1_800_000_000;
const DAY: i64 = 24 * 60 * 60;

fn checker(current: &str) -> UpdateChecker {
    UpdateChecker::new(current).expect("checker starts")
}

fn release(version: &str) -> Release {
    Release {
        version: Version::parse(version).expect("version is valid"),
        url: String::new(),
    }
}

/// A new, empty install prefix. `marker` adds the marker file.
fn prefix(name: &str, marker: bool) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ringo-update-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    if marker {
        let file = root.join(MARKER_FILE);
        fs::create_dir_all(file.parent().expect("file has a parent")).expect("dir is created");
        fs::write(file, "").expect("marker is written");
    }
    fs::create_dir_all(&root).expect("dir is created");
    root
}

#[test]
fn newer_version_is_detected() {
    let checker = checker("0.9.0");
    assert!(checker.is_newer(&release("0.9.1")));
    assert!(checker.is_newer(&release("1.0.0")));
}

#[test]
fn same_or_older_version_is_not_newer() {
    let checker = checker("0.9.0");
    assert!(!checker.is_newer(&release("0.9.0")));
    assert!(!checker.is_newer(&release("0.8.9")));
}

#[test]
fn release_is_newer_than_its_release_candidate() {
    assert!(checker("0.9.0-rc.1").is_newer(&release("0.9.0")));
    assert!(!checker("0.9.0").is_newer(&release("0.9.0-rc.1")));
}

#[test]
fn invalid_current_version_is_an_error() {
    assert!(matches!(
        UpdateChecker::new("nine"),
        Err(Error::InvalidVersion { .. })
    ));
}

#[test]
fn marker_file_is_detected() {
    let with = prefix("with", true);
    let without = prefix("without", false);
    assert!(disabled_by_package(&with));
    assert!(!disabled_by_package(&without));
    fs::remove_dir_all(with).expect("dir is removed");
    fs::remove_dir_all(without).expect("dir is removed");
}

#[test]
fn check_waits_24_hours() {
    let root = prefix("interval", false);
    assert!(should_check(true, &root, 0, NOW));
    assert!(!should_check(true, &root, NOW - DAY + 1, NOW));
    assert!(should_check(true, &root, NOW - DAY, NOW));
    fs::remove_dir_all(root).expect("dir is removed");
}

#[test]
fn check_runs_when_the_clock_moved_back() {
    let root = prefix("clock", false);
    assert!(should_check(true, &root, NOW + DAY, NOW));
    fs::remove_dir_all(root).expect("dir is removed");
}

#[test]
fn check_is_off_by_setting_or_by_marker() {
    let plain = prefix("off-setting", false);
    let marked = prefix("off-marker", true);
    assert!(!should_check(false, &plain, 0, NOW));
    assert!(!should_check(true, &marked, 0, NOW));
    fs::remove_dir_all(plain).expect("dir is removed");
    fs::remove_dir_all(marked).expect("dir is removed");
}

#[test]
fn recorded_response_is_parsed() {
    let release = Release::from_json(RECORDED_RESPONSE.as_bytes()).expect("response is valid");
    assert_eq!(release.version, Version::new(1, 2, 3));
    assert_eq!(
        release.url,
        "https://github.com/MedaiP90/RingoMusic/releases/tag/v1.2.3"
    );
}

#[test]
fn response_without_tag_is_an_error() {
    let result = Release::from_json(br#"{"html_url": "https://example.org"}"#);
    assert!(matches!(result, Err(Error::UpdateResponse(_))));
}

#[test]
fn response_with_bad_tag_is_an_error() {
    let result = Release::from_json(br#"{"tag_name": "nightly", "html_url": "x"}"#);
    assert!(matches!(result, Err(Error::InvalidVersion { .. })));
}
