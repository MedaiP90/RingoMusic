// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Application database: connection, migrations, and blocking-call helper.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use tokio::task;

use crate::api_cache::delete_expired;
use crate::{Error, Result};

/// Migration steps in order. Step `n` (counting from 1) moves the schema to version `n`.
/// Never change a released step. Add a new one.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE api_cache (
        key TEXT PRIMARY KEY,
        body TEXT NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    // `place` is the index in the user order (queue_item) or the position in the
    // shuffle order (queue_shuffle). `queue_state` has one row at most.
    "CREATE TABLE queue_item (
        place INTEGER PRIMARY KEY,
        id TEXT NOT NULL,
        title TEXT NOT NULL,
        artist TEXT NOT NULL,
        album TEXT NOT NULL,
        duration_ms INTEGER NOT NULL,
        artwork_template TEXT
    );
    CREATE TABLE queue_shuffle (
        place INTEGER PRIMARY KEY,
        item_index INTEGER NOT NULL
    );
    CREATE TABLE queue_history (
        place INTEGER PRIMARY KEY,
        id TEXT NOT NULL
    );
    CREATE TABLE queue_state (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        current_index INTEGER NOT NULL,
        repeat TEXT NOT NULL,
        position_ms INTEGER NOT NULL
    )",
    // `size` keeps the byte length of `body`. The covering index lets the size
    // and expiry queries skip the large bodies.
    "ALTER TABLE api_cache ADD COLUMN size INTEGER NOT NULL DEFAULT 0;
    UPDATE api_cache SET size = length(CAST(body AS BLOB));
    CREATE INDEX api_cache_expiry ON api_cache (expires_at, key, size)",
];

/// Handle to the application database. Clones share one connection.
#[derive(Clone, Debug)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens the database file at `path`, creates it if needed, and migrates it.
    /// Expired cache rows are deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened, or if a migration fails.
    pub fn open(path: &Path) -> Result<Self> {
        let open_error = |source| Error::Open {
            path: path.to_owned(),
            source,
        };
        let connection = Connection::open(path).map_err(open_error)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(open_error)?;
        Self::initialize(connection)
    }

    /// Opens a database in memory. It is for tests.
    ///
    /// # Errors
    ///
    /// Returns an error if a migration fails.
    pub fn open_in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }

    /// Runs `work` with the connection on a blocking thread.
    ///
    /// # Errors
    ///
    /// Returns the error of `work`, or [`Error::Task`] if the thread fails.
    ///
    /// # Panics
    ///
    /// Panics if an earlier call panicked while it held the connection.
    pub async fn run<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        task::spawn_blocking(move || {
            let mut connection = connection.lock().expect("connection mutex is not poisoned");
            work(&mut connection)
        })
        .await
        .map_err(Error::Task)?
    }

    fn initialize(mut connection: Connection) -> Result<Self> {
        migrate(&mut connection)?;
        delete_expired(&connection, unix_seconds(SystemTime::now()))?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }
}

/// Converts `time` to seconds since the Unix epoch. A time before the epoch gives 0.
pub(crate) fn unix_seconds(time: SystemTime) -> i64 {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    i64::try_from(seconds).unwrap_or(i64::MAX)
}

/// Applies each migration step that the database has not seen yet.
/// Each step and its version update are one transaction.
fn migrate(connection: &mut Connection) -> Result<()> {
    let supported = u32::try_from(MIGRATIONS.len()).expect("migration count fits in u32");
    let current: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > supported {
        return Err(Error::UnsupportedVersion {
            found: current,
            supported,
        });
    }
    for (index, step) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = u32::try_from(index + 1).expect("migration count fits in u32");
        apply_step(connection, step, version)
            .map_err(|source| Error::Migrate { version, source })?;
    }
    Ok(())
}

fn apply_step(connection: &mut Connection, step: &str, version: u32) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(step)?;
    transaction.pragma_update(None, "user_version", version)?;
    transaction.commit()
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::*;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        let path = env::temp_dir().join(format!("ringo-db-{}-{name}.db", process::id()));
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(format!("{}{suffix}", path.display()));
        }
        path
    }

    fn user_version(connection: &Connection) -> u32 {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("user_version can be read")
    }

    #[test]
    fn open_migrates_an_existing_database_at_version_zero() {
        let path = scratch_path("version-zero");
        let existing = Connection::open(&path).expect("file can be created");
        existing
            .execute_batch("CREATE TABLE other (id INTEGER); INSERT INTO other VALUES (7)")
            .expect("existing schema can be created");
        assert_eq!(user_version(&existing), 0);
        drop(existing);

        Database::open(&path).expect("database can be opened");

        let connection = Connection::open(&path).expect("file can be reopened");
        assert_eq!(user_version(&connection), 3);
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM api_cache", [], |row| row.get(0))
            .expect("api_cache exists");
        assert_eq!(rows, 0);
        let kept: i64 = connection
            .query_row("SELECT id FROM other", [], |row| row.get(0))
            .expect("existing data is kept");
        assert_eq!(kept, 7);
    }

    #[test]
    fn open_turns_on_wal_mode() {
        let path = scratch_path("wal");
        Database::open(&path).expect("database can be opened");

        let connection = Connection::open(&path).expect("file can be reopened");
        let mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("journal_mode can be read");
        assert_eq!(mode, "wal");
    }

    #[test]
    fn open_rejects_a_database_from_a_newer_version() {
        let path = scratch_path("newer");
        let existing = Connection::open(&path).expect("file can be created");
        existing
            .pragma_update(None, "user_version", 99)
            .expect("version can be set");
        drop(existing);

        let error = Database::open(&path).expect_err("version is too new");

        assert!(matches!(
            error,
            Error::UnsupportedVersion {
                found: 99,
                supported: 3
            }
        ));
    }

    #[test]
    fn open_deletes_expired_cache_rows_and_keeps_valid_ones() {
        let path = scratch_path("purge");
        Database::open(&path).expect("database can be opened");
        let connection = Connection::open(&path).expect("file can be reopened");
        let future = unix_seconds(SystemTime::now()) + 3600;
        connection
            .execute_batch(&format!(
                "INSERT INTO api_cache (key, body, expires_at) VALUES ('old', '1', 1), ('new', '2', {future})"
            ))
            .expect("rows can be inserted");
        drop(connection);

        Database::open(&path).expect("database can be reopened");

        let connection = Connection::open(&path).expect("file can be reopened");
        let key: String = connection
            .query_row("SELECT key FROM api_cache", [], |row| row.get(0))
            .expect("one row is left");
        assert_eq!(key, "new");
    }

    #[test]
    fn migration_three_sets_the_size_of_existing_cache_rows() {
        let path = scratch_path("size");
        let mut existing = Connection::open(&path).expect("file can be created");
        for step in &MIGRATIONS[..2] {
            existing.execute_batch(step).expect("old step can run");
        }
        existing
            .pragma_update(None, "user_version", 2)
            .expect("version can be set");
        let future = unix_seconds(SystemTime::now()) + 3600;
        existing
            .execute("INSERT INTO api_cache VALUES ('key', 'àbc', ?1)", [future])
            .expect("row can be inserted");
        migrate(&mut existing).expect("migration runs");

        let size: i64 = existing
            .query_row("SELECT size FROM api_cache", [], |row| row.get(0))
            .expect("one row is left");
        assert_eq!(size, 4);
    }
}
