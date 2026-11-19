// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Command-line client for the player.

use clap::Parser;
use tracing_subscriber::EnvFilter;

/// Command-line arguments of `ringo-cli`.
#[derive(Debug, Parser)]
#[command(name = "ringo-cli", version)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
    init_tracing();
    tracing::info!("ringo-cli started");
}

/// Sets up logging from `RUST_LOG`. The default level is `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
