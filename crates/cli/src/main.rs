// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Developer command-line client for the playback engine.

mod event_json;
mod play;
mod search;

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ringo_am_api::ApiTransport;
use ringo_core::paths::Paths;
use ringo_engine::{EngineConfig, EngineEvent, PlaybackEngine, SidecarEngine, SidecarTransport};
use serde_json::Value;
use tokio::signal;
use tokio::sync::broadcast::{self, error::RecvError};
use tracing_subscriber::EnvFilter;

/// Text that explains how to run the engine during development.
const DEVELOPMENT_HELP: &str = "\
Development setup:
  1. Build the engine first:
       cd engine && npm ci && npm run build
  2. Tell ringo-cli how to start it (paths are relative to the repository root):
       export RINGO_ENGINE_CMD=\"$PWD/engine/node_modules/.bin/electron\"
       export RINGO_ENGINE_ARGS=\"$PWD/engine/\"
  3. XDG_RUNTIME_DIR must be set. The engine data is in
     $XDG_DATA_HOME/ringomusic/engine (default ~/.local/share/ringomusic/engine).";

/// Command-line arguments of `ringo-cli`.
#[derive(Debug, Parser)]
#[command(name = "ringo-cli", version, after_help = DEVELOPMENT_HELP)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Command that `ringo-cli` runs.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the authorization state and the storefront.
    Status,
    /// Show the login window and print the result.
    Signin,
    /// Sign the user out.
    Signout,
    /// Search the catalog for songs, albums, and artists.
    Search {
        /// Text to search for.
        term: String,
    },
    /// Print the raw JSON of an API GET request. `{storefront}` in the path is replaced.
    ApiGet {
        /// API path, for example `/v1/catalog/{storefront}/songs/1440935467`.
        path: String,
        /// Query parameters.
        #[arg(value_name = "KEY=VALUE")]
        params: Vec<String>,
    },
    /// Play songs and print the player events. Restore playback after an engine restart.
    Play {
        /// Catalog or library song IDs, in play order.
        #[arg(required = true)]
        song_ids: Vec<String>,
    },
    /// Print every engine event as one JSON line.
    Events,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing();
    let paths = Paths::from_env().context("cannot resolve the application paths")?;
    paths
        .ensure()
        .context("cannot create the application directories")?;
    let config = EngineConfig::from_env_or_default(
        paths.engine_data_dir,
        paths.runtime_dir.join("engine.sock"),
    )
    .context("cannot configure the engine (see --help for the development setup)")?;
    let engine = Arc::new(SidecarEngine::start(config).context("cannot start the engine")?);
    // The runtime has one thread, so no event is lost between `start` and `subscribe`.
    let events = engine.events();

    let result = tokio::select! {
        result = run(&engine, cli.command, events) => result,
        _ = signal::ctrl_c() => Ok(()),
    };
    engine.shutdown().await.context("cannot stop the engine")?;
    result
}

/// Sets up logging from `RUST_LOG`. The default level is `warn`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Runs one command against the engine.
async fn run(
    engine: &Arc<SidecarEngine>,
    command: Command,
    mut events: broadcast::Receiver<EngineEvent>,
) -> Result<()> {
    if matches!(command, Command::Events) {
        return print_events(&mut events).await;
    }
    wait_until_ready(&mut events).await?;
    let transport = SidecarTransport::new(Arc::clone(engine));
    let engine: &SidecarEngine = engine;
    match command {
        Command::Status => print_status(engine).await,
        Command::Signin => {
            engine.sign_in().await?;
            print_status(engine).await
        }
        Command::Signout => Ok(engine.sign_out().await?),
        Command::Search { term } => search_catalog(engine, &transport, &term).await,
        Command::ApiGet { path, params } => {
            let response = api_get(engine, &transport, &path, &params).await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
            Ok(())
        }
        Command::Play { song_ids } => play::play(engine, song_ids, events).await,
        Command::Events => unreachable!("internal error: handled before the engine is ready"),
    }
}

/// Waits for `engine.ready`. Fails if the engine is incompatible or gives up.
async fn wait_until_ready(events: &mut broadcast::Receiver<EngineEvent>) -> Result<()> {
    loop {
        match events.recv().await {
            Ok(EngineEvent::Ready) => return Ok(()),
            Ok(EngineEvent::Incompatible { check, detail }) => {
                bail!("engine is incompatible: {check}: {detail}");
            }
            Ok(EngineEvent::GaveUp) => bail!("engine did not start"),
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => bail!("engine stopped"),
        }
    }
}

/// Prints each event as one JSON line until the engine gives up.
async fn print_events(events: &mut broadcast::Receiver<EngineEvent>) -> Result<()> {
    loop {
        match events.recv().await {
            Ok(event) => {
                println!("{}", event_json::to_json(&event));
                if event == EngineEvent::GaveUp {
                    return Ok(());
                }
            }
            Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return Ok(()),
        }
    }
}

async fn print_status(engine: &SidecarEngine) -> Result<()> {
    let status = engine.auth_status().await?;
    println!("authorized: {}", status.authorized);
    println!("storefront: {}", status.storefront);
    Ok(())
}

async fn search_catalog(
    engine: &SidecarEngine,
    transport: &impl ApiTransport,
    term: &str,
) -> Result<()> {
    let params = [
        format!("term={term}"),
        "types=songs,albums,artists".to_owned(),
        "limit=10".to_owned(),
    ];
    let response = api_get(
        engine,
        transport,
        "/v1/catalog/{storefront}/search",
        &params,
    )
    .await?;
    search::print_table(&search::rows(&response));
    Ok(())
}

/// Sends a GET request. Fills `{storefront}` in the path and parses `key=value` pairs.
async fn api_get(
    engine: &SidecarEngine,
    transport: &impl ApiTransport,
    path: &str,
    params: &[String],
) -> Result<Value> {
    let query = parse_params(params)?;
    let path = if path.contains("{storefront}") {
        let status = engine.auth_status().await?;
        path.replace("{storefront}", &status.storefront)
    } else {
        path.to_owned()
    };
    Ok(transport.get(&path, &query).await?)
}

/// Splits each `key=value` pair at the first `=`.
fn parse_params(params: &[String]) -> Result<Vec<(&str, &str)>> {
    params
        .iter()
        .map(|param| match param.split_once('=') {
            Some(pair) => Ok(pair),
            None => bail!("query parameter '{param}' is not in the form key=value"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_params_splits_at_the_first_equals_sign() {
        let input = ["term=a=b".to_owned(), "limit=10".to_owned()];
        let params = parse_params(&input).expect("params are valid");
        assert_eq!(params, [("term", "a=b"), ("limit", "10")]);
    }

    #[test]
    fn parse_params_rejects_a_parameter_without_equals_sign() {
        assert!(parse_params(&["limit".to_owned()]).is_err());
    }
}
