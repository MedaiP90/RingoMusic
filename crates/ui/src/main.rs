// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Desktop application for the Apple Music player.

mod about;
mod app;
mod artwork;
mod background;
mod debug_play;
mod gate;
mod item;
mod item_actions;
mod menu;
mod network_banner;
mod notification;
mod paged;
mod playback_actions;
mod section;
mod services;
mod sidebar;
mod startup_error;
mod subscription;
mod views;
mod widgets;

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use relm4::RelmApp;
use relm4::adw;
use relm4::gtk::gio;
use relm4::gtk::prelude::*;
use ringo_core::paths::{PackageFormat, Paths};
use tracing_subscriber::EnvFilter;

use crate::app::{App, Init};
use crate::services::Services;
use crate::startup_error::StartupError;

/// Application ID. It is also the name of the desktop file.
const APP_ID: &str = "io.github.MedaiP90.RingoMusic";

/// Gettext text domain. The catalogs are `<prefix>/share/locale/<language>/LC_MESSAGES/ringomusic.mo`.
const TEXT_DOMAIN: &str = "ringomusic";

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
    gio::resources_register_include!("ringomusic.gresource")
        .context("cannot register the resource bundle")?;

    let app = adw::Application::builder().application_id(APP_ID).build();
    match start_services() {
        Ok(services) => run_shell(app, services),
        Err(error) => {
            tracing::error!("cannot start: {error:#}");
            RelmApp::from_app(app).run::<StartupError>(format!("{error:#}"));
        }
    }
    Ok(())
}

/// Resolves the paths, sets up the translations, and starts the services.
fn start_services() -> anyhow::Result<Services> {
    let paths = Paths::from_env().context("cannot resolve the application paths")?;
    setup_gettext(&paths);
    Services::start(paths).context("cannot start the core and the engine")
}

/// Runs the main window until the application quits.
fn run_shell(app: adw::Application, services: Services) {
    let init = Init {
        services: services.shared().clone(),
        package: PackageFormat::detect(),
    };
    debug_play::spawn(services.shared().core().clone());
    // The core stops the engine. The application waits for it, so no engine process stays.
    let services = Rc::new(RefCell::new(Some(services)));
    app.connect_shutdown(move |_| {
        if let Some(services) = services.borrow_mut().take() {
            services.shutdown();
        }
    });
    RelmApp::from_app(app).run::<App>(init);
}

/// Selects the text domain and its catalog directory. A failure only leaves
/// the texts in English.
fn setup_gettext(paths: &Paths) {
    let locale_dir = paths.install_prefix.join("share/locale");
    let result = bindtextdomain(TEXT_DOMAIN, locale_dir)
        .and_then(|_| bind_textdomain_codeset(TEXT_DOMAIN, "UTF-8"))
        .and_then(|_| textdomain(TEXT_DOMAIN));
    if let Err(error) = result {
        tracing::warn!(%error, "cannot set up the translations");
    }
}

/// Sets up logging from `RUST_LOG`. The default level is `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
