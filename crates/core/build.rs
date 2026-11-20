// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Compiles the `GSettings` schema into `target/` for development and tests.

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const SCHEMA_DIR: &str = "../../data";

fn main() {
    println!("cargo::rerun-if-changed={SCHEMA_DIR}");

    let out_dir =
        PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR for build scripts"));
    let compiled_dir = out_dir.join("schemas");
    fs::create_dir_all(&compiled_dir).expect("OUT_DIR is writable");

    let status = Command::new("glib-compile-schemas")
        .arg("--strict")
        .arg(format!("--targetdir={}", compiled_dir.display()))
        .arg(SCHEMA_DIR)
        .status()
        .expect("glib-compile-schemas is installed (package libglib2.0-bin)");
    assert!(status.success(), "glib-compile-schemas failed: {status}");

    println!(
        "cargo::rustc-env=RINGO_SCHEMA_DIR={}",
        compiled_dir.display()
    );
}
