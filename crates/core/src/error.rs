// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::path::PathBuf;

/// Error of the core storage.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The database file cannot be opened or configured.
    #[error("cannot open database {path}")]
    Open {
        /// Path of the database file.
        path: PathBuf,
        /// Cause of the failure.
        #[source]
        source: rusqlite::Error,
    },

    /// A migration step failed. The step is rolled back.
    #[error("cannot migrate database to version {version}")]
    Migrate {
        /// Schema version that the step creates.
        version: u32,
        /// Cause of the failure.
        #[source]
        source: rusqlite::Error,
    },

    /// The database was written by a newer version of the application.
    #[error("database version {found} is newer than supported version {supported}")]
    UnsupportedVersion {
        /// Version stored in the database.
        found: u32,
        /// Highest version that this build knows.
        supported: u32,
    },

    /// An SQL statement failed.
    #[error("database query failed")]
    Query(#[from] rusqlite::Error),

    /// A required environment variable is not set.
    #[error("environment variable {name} is not set")]
    MissingEnvironment {
        /// Name of the variable.
        name: &'static str,
    },

    /// The path of the running executable is unknown.
    #[error("cannot find the path of the executable")]
    Executable(#[source] std::io::Error),

    /// An application directory cannot be created.
    #[error("cannot create directory {path}")]
    CreateDir {
        /// Path of the directory.
        path: PathBuf,
        /// Cause of the failure.
        #[source]
        source: std::io::Error,
    },

    /// The blocking task of a database call did not finish.
    #[error("database task failed")]
    Task(#[source] tokio::task::JoinError),
}

/// Result type of the core storage.
pub type Result<T, E = Error> = std::result::Result<T, E>;
