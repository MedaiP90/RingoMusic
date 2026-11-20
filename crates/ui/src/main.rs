// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Desktop application for the Apple Music player.

mod debug_play;
mod main_window;
mod services;

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use relm4::RelmApp;
use relm4::adw;
use relm4::gtk::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::main_window::MainWindow;
use crate::services::Services;

/// Application ID. It is also the name of the desktop file.
const APP_ID: &str = "io.github.MedaiP90.RingoMusic";

/// Returns true when the command line asks for the version.
fn wants_version() -> bool {
    std::env::args()
        .skip(1)
        .any(|argument| argument == "--version")
}

fn main() -> anyhow::Result<()> {
    if wants_version() {
        println!("ringomusic {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    init_tracing();
    tracing::info!("ringomusic started");

    // ponytail: a start failure ends the program with a message on stderr. The UI
    // prompts add an error page.
    let services = Services::start().context("cannot start the core and the engine")?;
    let core = services.core().clone();
    debug_play::spawn(core.clone());

    let app = adw::Application::builder().application_id(APP_ID).build();
    // The core stops the engine. The application waits for it, so no engine process stays.
    let services = Rc::new(RefCell::new(Some(services)));
    app.connect_shutdown(move |_| {
        if let Some(services) = services.borrow_mut().take() {
            services.shutdown();
        }
    });
    RelmApp::from_app(app).run::<MainWindow>(core);
    Ok(())
}

/// Sets up logging from `RUST_LOG`. The default level is `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
