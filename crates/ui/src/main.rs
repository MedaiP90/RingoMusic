// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Desktop application for the Apple Music player.

use tracing_subscriber::EnvFilter;

/// Returns true when the command line asks for the version.
fn wants_version() -> bool {
    std::env::args()
        .skip(1)
        .any(|argument| argument == "--version")
}

fn main() {
    if wants_version() {
        println!("ringomusic {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    init_tracing();
    tracing::info!("ringomusic started");
}

/// Sets up logging from `RUST_LOG`. The default level is `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
