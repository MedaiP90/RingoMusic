// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Result, bail};

/// Returns the data directory of the engine.
///
/// ponytail: the CLI decides the path here. Prompt 20 moves path logic to the core crate.
pub fn engine_data_dir() -> Result<PathBuf> {
    resolve(env::var_os("XDG_DATA_HOME"), env::var_os("HOME"))
}

fn resolve(data_home: Option<OsString>, home: Option<OsString>) -> Result<PathBuf> {
    let base = match (data_home.filter(|dir| !dir.is_empty()), home) {
        (Some(data_home), _) => PathBuf::from(data_home),
        (None, Some(home)) => PathBuf::from(home).join(".local/share"),
        (None, None) => bail!("neither XDG_DATA_HOME nor HOME is set"),
    };
    Ok(base.join("ringomusic/engine"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_data_home_wins_over_home() {
        let path = resolve(Some("/data".into()), Some("/home/user".into())).expect("path");
        assert_eq!(path, PathBuf::from("/data/ringomusic/engine"));
    }

    #[test]
    fn home_is_the_fallback() {
        let path = resolve(Some("".into()), Some("/home/user".into())).expect("path");
        assert_eq!(
            path,
            PathBuf::from("/home/user/.local/share/ringomusic/engine")
        );
    }

    #[test]
    fn missing_variables_are_an_error() {
        assert!(resolve(None, None).is_err());
    }
}
