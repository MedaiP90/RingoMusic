// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Environment variable that overrides the engine command.
const COMMAND_ENV: &str = "RINGO_ENGINE_CMD";
/// Environment variable with extra arguments, split at white space.
const ARGS_ENV: &str = "RINGO_ENGINE_ARGS";
/// Engine path relative to the directory of the executable (decision D23).
const DEFAULT_ENGINE_PATH: &str = "../lib/ringomusic/engine/ringomusic-engine";

/// How to start the engine process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineConfig {
    /// Program to start.
    pub command: PathBuf,
    /// Arguments of the program.
    pub args: Vec<OsString>,
    /// Path of the Unix socket that Rust creates.
    pub socket_path: PathBuf,
    /// Data directory of the engine.
    pub data_dir: PathBuf,
}

impl EngineConfig {
    /// Reads `RINGO_ENGINE_CMD` and `RINGO_ENGINE_ARGS`, and uses defaults for the rest.
    ///
    /// The default command is relative to the current executable.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if an environment variable is not valid UTF-8, and
    /// [`Error::Io`] if the executable path is unknown.
    pub fn from_env_or_default(data_dir: PathBuf, socket_path: PathBuf) -> Result<Self> {
        let executable = env::current_exe()?;
        Self::from_parts(
            env::var_os(COMMAND_ENV),
            env::var_os(ARGS_ENV),
            &executable,
            data_dir,
            socket_path,
        )
    }

    fn from_parts(
        command: Option<OsString>,
        args: Option<OsString>,
        executable: &Path,
        data_dir: PathBuf,
        socket_path: PathBuf,
    ) -> Result<Self> {
        let command = match command {
            Some(command) => PathBuf::from(command),
            None => default_command(executable)?,
        };
        let args = match args {
            Some(args) => split_args(args)?,
            None => Vec::new(),
        };
        Ok(Self {
            command,
            args,
            socket_path,
            data_dir,
        })
    }
}

fn default_command(executable: &Path) -> Result<PathBuf> {
    let directory = executable.parent().ok_or_else(|| Error::Config {
        detail: "the executable has no parent directory".to_owned(),
    })?;
    Ok(directory.join(DEFAULT_ENGINE_PATH))
}

fn split_args(args: OsString) -> Result<Vec<OsString>> {
    let args = args.into_string().map_err(|_| Error::Config {
        detail: format!("{ARGS_ENV} is not valid UTF-8"),
    })?;
    Ok(args.split_whitespace().map(OsString::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(command: Option<&str>, args: Option<&str>) -> Result<EngineConfig> {
        EngineConfig::from_parts(
            command.map(OsString::from),
            args.map(OsString::from),
            Path::new("/usr/bin/ringomusic"),
            PathBuf::from("/data"),
            PathBuf::from("/run/user/1000/ringomusic/engine.sock"),
        )
    }

    #[test]
    fn default_command_is_relative_to_the_executable() {
        let config = parts(None, None).expect("config is valid");
        assert_eq!(
            config.command,
            Path::new("/usr/bin/../lib/ringomusic/engine/ringomusic-engine")
        );
        assert_eq!(
            config.socket_path,
            Path::new("/run/user/1000/ringomusic/engine.sock")
        );
        assert_eq!(config.data_dir, Path::new("/data"));
    }

    #[test]
    fn environment_overrides_command_and_args() {
        let config = parts(Some("electron"), Some("engine/ --foo")).expect("config is valid");
        assert_eq!(config.command, Path::new("electron"));
        assert_eq!(config.args, ["engine/", "--foo"]);
    }
}
