// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! File locations of the application and the package format.

use std::ffi::OsString;
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{env, io};

use crate::{Error, Result};

/// File that exists inside a Flatpak sandbox.
const FLATPAK_MARKER: &str = "/.flatpak-info";
/// Name of the application directory inside each XDG base directory.
const APP_DIR: &str = "ringomusic";

/// How the application is installed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PackageFormat {
    /// Runs inside a Flatpak sandbox.
    Flatpak,
    /// Runs on the host system.
    Native,
}

impl PackageFormat {
    /// Detects the package format of the running process.
    #[must_use]
    pub fn detect() -> Self {
        if Path::new(FLATPAK_MARKER).exists() {
            Self::Flatpak
        } else {
            Self::Native
        }
    }
}

/// All directories and files that the application uses.
///
/// Flatpak sets the XDG variables to `~/.var/app/<app id>/...`, so the same code
/// gives the correct paths for both package formats.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paths {
    /// `$XDG_DATA_HOME/ringomusic`.
    pub data_dir: PathBuf,
    /// SQLite database file.
    pub database: PathBuf,
    /// Data directory of the playback engine.
    pub engine_data_dir: PathBuf,
    /// `$XDG_CACHE_HOME/ringomusic`.
    pub cache_dir: PathBuf,
    /// Cache of downloaded artwork.
    pub artwork_cache_dir: PathBuf,
    /// `$XDG_RUNTIME_DIR/ringomusic`. Holds sockets.
    pub runtime_dir: PathBuf,
    /// Parent of the directory that holds the executable.
    pub install_prefix: PathBuf,
}

impl Paths {
    /// Resolves the paths from the process environment and the current executable.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingEnvironment`] if `XDG_RUNTIME_DIR` is not set, or if
    /// `HOME` is needed and not set. Returns [`Error::Executable`] if the path of the
    /// executable is unknown.
    pub fn from_env() -> Result<Self> {
        let executable = env::current_exe().map_err(Error::Executable)?;
        Self::resolve(|name| env::var_os(name), &executable)
    }

    /// Resolves the paths with `lookup` instead of the process environment.
    ///
    /// An empty value counts as not set.
    ///
    /// # Errors
    ///
    /// See [`Paths::from_env`].
    pub fn resolve(lookup: impl Fn(&str) -> Option<OsString>, executable: &Path) -> Result<Self> {
        let var = |name: &str| lookup(name).filter(|value| !value.is_empty());
        let home = || {
            var("HOME")
                .map(PathBuf::from)
                .ok_or(Error::MissingEnvironment { name: "HOME" })
        };
        let xdg_dir = |name: &str, fallback: &str| match var(name) {
            Some(dir) => Ok(PathBuf::from(dir)),
            None => home().map(|home| home.join(fallback)),
        };

        let data_dir = xdg_dir("XDG_DATA_HOME", ".local/share")?.join(APP_DIR);
        let cache_dir = xdg_dir("XDG_CACHE_HOME", ".cache")?.join(APP_DIR);
        let runtime_dir = var("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join(APP_DIR))
            .ok_or(Error::MissingEnvironment {
                name: "XDG_RUNTIME_DIR",
            })?;
        let install_prefix = executable
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                Error::Executable(io::Error::other("executable has no grandparent directory"))
            })?
            .to_path_buf();

        Ok(Self {
            database: data_dir.join("ringomusic.db"),
            engine_data_dir: data_dir.join("engine"),
            artwork_cache_dir: cache_dir.join("artwork"),
            data_dir,
            cache_dir,
            runtime_dir,
            install_prefix,
        })
    }

    /// Creates the directories. `runtime_dir` gets mode `0700`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CreateDir`] for the first directory that cannot be created.
    pub fn ensure(&self) -> Result<()> {
        for dir in [
            &self.engine_data_dir,
            &self.artwork_cache_dir,
            &self.runtime_dir,
        ] {
            fs::create_dir_all(dir).map_err(|source| Error::CreateDir {
                path: dir.clone(),
                source,
            })?;
        }
        fs::set_permissions(&self.runtime_dir, Permissions::from_mode(0o700)).map_err(|source| {
            Error::CreateDir {
                path: self.runtime_dir.clone(),
                source,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        let pairs = pairs.to_vec();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    fn resolve(pairs: &[(&'static str, &'static str)]) -> Result<Paths> {
        Paths::resolve(lookup(pairs), Path::new("/usr/bin/ringomusic"))
    }

    #[test]
    fn xdg_variables_set_the_paths() {
        let paths = resolve(&[
            ("XDG_DATA_HOME", "/data"),
            ("XDG_CACHE_HOME", "/cache"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ])
        .expect("paths are valid");
        assert_eq!(paths.data_dir, Path::new("/data/ringomusic"));
        assert_eq!(paths.database, Path::new("/data/ringomusic/ringomusic.db"));
        assert_eq!(paths.engine_data_dir, Path::new("/data/ringomusic/engine"));
        assert_eq!(paths.cache_dir, Path::new("/cache/ringomusic"));
        assert_eq!(
            paths.artwork_cache_dir,
            Path::new("/cache/ringomusic/artwork")
        );
        assert_eq!(paths.runtime_dir, Path::new("/run/user/1000/ringomusic"));
        assert_eq!(paths.install_prefix, Path::new("/usr"));
    }

    #[test]
    fn home_is_the_fallback_for_data_and_cache() {
        let paths = resolve(&[
            ("HOME", "/home/user"),
            ("XDG_DATA_HOME", ""),
            ("XDG_RUNTIME_DIR", "/run"),
        ])
        .expect("paths are valid");
        assert_eq!(
            paths.data_dir,
            Path::new("/home/user/.local/share/ringomusic")
        );
        assert_eq!(paths.cache_dir, Path::new("/home/user/.cache/ringomusic"));
    }

    #[test]
    fn missing_runtime_dir_is_an_error() {
        let result = resolve(&[("HOME", "/home/user")]);
        assert!(matches!(
            result,
            Err(Error::MissingEnvironment {
                name: "XDG_RUNTIME_DIR"
            })
        ));
    }

    #[test]
    fn missing_home_without_xdg_variables_is_an_error() {
        let result = resolve(&[("XDG_RUNTIME_DIR", "/run")]);
        assert!(matches!(
            result,
            Err(Error::MissingEnvironment { name: "HOME" })
        ));
    }

    #[test]
    fn ensure_creates_directories_with_private_runtime_dir() {
        let root = env::temp_dir().join(format!("ringo-paths-test-{}", std::process::id()));
        let root_text = root.to_str().expect("temp path is UTF-8").to_owned();
        let leaked: &'static str = Box::leak(root_text.into_boxed_str());
        let paths = resolve(&[
            ("XDG_DATA_HOME", leaked),
            ("XDG_CACHE_HOME", leaked),
            ("XDG_RUNTIME_DIR", leaked),
        ])
        .expect("paths are valid");

        paths.ensure().expect("directories are created");

        assert!(paths.engine_data_dir.is_dir());
        assert!(paths.artwork_cache_dir.is_dir());
        let mode = fs::metadata(&paths.runtime_dir)
            .expect("runtime dir exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        fs::remove_dir_all(root).expect("test directory is removed");
    }
}
