// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Cache of Apple Music catalog responses.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use ringo_am_api::{ApiTransport, Result};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use tokio::task::JoinHandle;
use tokio::time::{self, Instant};

use crate::db::{Database, unix_seconds};

const TEN_MINUTES: Duration = Duration::from_mins(10);
const HOUR: Duration = Duration::from_hours(1);
const SEVEN_DAYS: Duration = Duration::from_hours(168);

const CATALOG_PREFIX: &str = "/v1/catalog/";

/// Time to keep a response, by resource type. The type is the path segment after the storefront.
const EXPIRY_BY_RESOURCE: &[(&str, Duration)] = &[
    ("songs", SEVEN_DAYS),
    ("albums", SEVEN_DAYS),
    ("artists", SEVEN_DAYS),
    ("playlists", HOUR),
    ("search", TEN_MINUTES),
];

/// Time to keep a catalog response of any other resource type.
const DEFAULT_EXPIRY: Duration = HOUR;

/// Default limit for the total size of the cached bodies.
pub const DEFAULT_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Default time between two sweeps of expired rows.
pub const SWEEP_INTERVAL: Duration = HOUR;

/// Source of the current time. Tests replace it to control expiry.
pub trait Clock: Send + Sync {
    /// Returns the current time.
    fn now(&self) -> SystemTime;
}

/// [`Clock`] that reads the system time.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// [`ApiTransport`] that caches catalog responses in the database.
///
/// Requests for other paths, such as the library, go to the inner transport.
/// A cache failure is logged and never fails a request.
pub struct CachingTransport<T, C = SystemClock> {
    inner: T,
    database: Database,
    clock: Arc<C>,
    max_bytes: u64,
}

impl<T: ApiTransport> CachingTransport<T> {
    /// Creates a transport that uses the system clock.
    #[must_use]
    pub fn new(inner: T, database: Database) -> Self {
        Self::with_clock(inner, database, SystemClock)
    }
}

impl<T: ApiTransport, C: Clock + 'static> CachingTransport<T, C> {
    /// Creates a transport that reads the time from `clock`.
    #[must_use]
    pub fn with_clock(inner: T, database: Database, clock: C) -> Self {
        Self {
            inner,
            database,
            clock: Arc::new(clock),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Sets the limit for the total size of the cached bodies.
    /// A write that exceeds it deletes the rows that expire first.
    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Starts a task that deletes expired rows every `interval`. It must run inside a
    /// Tokio runtime. Abort the returned handle to stop the task.
    #[must_use]
    pub fn spawn_sweeper(&self, interval: Duration) -> JoinHandle<()> {
        let database = self.database.clone();
        let clock = Arc::clone(&self.clock);
        tokio::spawn(async move {
            // Skip the first tick: opening the database already deleted expired rows.
            let mut ticks = time::interval_at(Instant::now() + interval, interval);
            loop {
                ticks.tick().await;
                sweep(&database, clock.as_ref()).await;
            }
        })
    }

    /// Returns the total size of the cached bodies, in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn used_bytes(&self) -> crate::Result<u64> {
        let bytes = self
            .database
            .run(|connection| Ok(body_bytes(connection)?))
            .await?;
        Ok(u64::try_from(bytes).unwrap_or_default())
    }

    /// Deletes all cached responses. Returns the number of freed bytes of body text.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn clear(&self) -> crate::Result<u64> {
        let freed = self
            .database
            .run(|connection| {
                let bytes = body_bytes(connection)?;
                connection.execute("DELETE FROM api_cache", [])?;
                Ok(bytes)
            })
            .await?;
        Ok(u64::try_from(freed).unwrap_or_default())
    }

    async fn read_cache(&self, key: &str) -> Option<Value> {
        let now = unix_seconds(self.clock.now());
        let key = key.to_owned();
        let body = self
            .database
            .run(move |connection| {
                Ok(connection
                    .query_row(
                        "SELECT body FROM api_cache WHERE key = ?1 AND expires_at > ?2",
                        (&key, now),
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?)
            })
            .await;
        match body {
            Ok(Some(body)) => serde_json::from_str(&body)
                .inspect_err(|error| tracing::warn!(%error, "cached response is not valid JSON"))
                .ok(),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, "cannot read the API cache");
                None
            }
        }
    }

    async fn write_cache(&self, key: String, value: &Value, lifetime: Duration) {
        let expires_at = unix_seconds(self.clock.now() + lifetime);
        let body = value.to_string();
        let max_bytes = i64::try_from(self.max_bytes).unwrap_or(i64::MAX);
        let now = unix_seconds(self.clock.now());
        let result = self
            .database
            .run(move |connection| {
                connection.execute(
                    "INSERT OR REPLACE INTO api_cache (key, body, expires_at) VALUES (?1, ?2, ?3)",
                    (&key, &body, expires_at),
                )?;
                delete_expired(connection, now)?;
                evict_over_limit(connection, max_bytes)?;
                Ok(())
            })
            .await;
        if let Err(error) = result {
            tracing::warn!(%error, "cannot write the API cache");
        }
    }
}

#[async_trait::async_trait]
impl<T: ApiTransport, C: Clock + 'static> ApiTransport for CachingTransport<T, C> {
    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let Some(lifetime) = expiry_for(path) else {
            return self.inner.get(path, query).await;
        };
        let key = cache_key(path, query);
        if let Some(value) = self.read_cache(&key).await {
            return Ok(value);
        }
        let value = self.inner.get(path, query).await?;
        self.write_cache(key, &value, lifetime).await;
        Ok(value)
    }
}

/// Deletes the expired rows of the cache with one database call.
async fn sweep(database: &Database, clock: &impl Clock) {
    let now = unix_seconds(clock.now());
    let result = database
        .run(move |connection| Ok(delete_expired(connection, now)?))
        .await;
    if let Err(error) = result {
        tracing::warn!(%error, "cannot sweep the API cache");
    }
}

/// Returns the total size of the cached bodies in bytes.
fn body_bytes(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row(
        "SELECT COALESCE(SUM(length(CAST(body AS BLOB))), 0) FROM api_cache",
        [],
        |row| row.get(0),
    )
}

/// Deletes the rows that are expired at `now` (seconds since the Unix epoch).
pub(crate) fn delete_expired(connection: &Connection, now: i64) -> rusqlite::Result<usize> {
    connection.execute("DELETE FROM api_cache WHERE expires_at <= ?1", [now])
}

/// Keeps the rows that expire last, up to `max_bytes` of body text, and deletes the rest.
fn evict_over_limit(connection: &Connection, max_bytes: i64) -> rusqlite::Result<usize> {
    connection.execute(
        "DELETE FROM api_cache WHERE key IN (
            SELECT key FROM (
                SELECT key,
                       SUM(length(CAST(body AS BLOB))) OVER (ORDER BY expires_at DESC, key) AS running
                FROM api_cache
            ) WHERE running > ?1
        )",
        [max_bytes],
    )
}

/// Returns how long to keep the response for `path`, or `None` if the path is not cached.
fn expiry_for(path: &str) -> Option<Duration> {
    let rest = path.strip_prefix(CATALOG_PREFIX)?;
    let resource = rest.split('/').nth(1)?;
    let expiry = EXPIRY_BY_RESOURCE
        .iter()
        .find(|(name, _)| *name == resource)
        .map_or(DEFAULT_EXPIRY, |&(_, expiry)| expiry);
    Some(expiry)
}

/// Builds the cache key from the path and the query sorted by key.
/// The key is JSON, so no value can make two different requests share a key.
fn cache_key(path: &str, query: &[(&str, &str)]) -> String {
    let mut sorted = query.to_vec();
    sorted.sort_unstable();
    serde_json::json!([path, sorted]).to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use ringo_am_api::{Error, TransportError};
    use serde_json::json;

    use super::*;

    /// Inner transport that counts calls and answers with the call number.
    #[derive(Clone, Default)]
    struct CountingTransport {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl ApiTransport for CountingTransport {
        async fn get(&self, _path: &str, _query: &[(&str, &str)]) -> Result<Value> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(json!({ "call": call }))
        }
    }

    /// Clock that moves only when the test says so.
    #[derive(Clone, Default)]
    struct ManualClock {
        seconds: Arc<AtomicU64>,
    }

    impl ManualClock {
        fn advance(&self, duration: Duration) {
            self.seconds.fetch_add(duration.as_secs(), Ordering::SeqCst);
        }
    }

    impl Clock for ManualClock {
        fn now(&self) -> SystemTime {
            // Start far from the epoch so the system purge at open keeps the rows.
            SystemTime::now() + Duration::from_secs(self.seconds.load(Ordering::SeqCst))
        }
    }

    struct Fixture {
        transport: CachingTransport<CountingTransport, ManualClock>,
        inner: CountingTransport,
        clock: ManualClock,
        database: Database,
    }

    fn fixture() -> Fixture {
        let inner = CountingTransport::default();
        let clock = ManualClock::default();
        let database = Database::open_in_memory().expect("database can be opened");
        let transport =
            CachingTransport::with_clock(inner.clone(), database.clone(), clock.clone());
        Fixture {
            transport,
            inner,
            clock,
            database,
        }
    }

    fn calls(fixture: &Fixture) -> usize {
        fixture.inner.calls.load(Ordering::SeqCst)
    }

    #[tokio::test]
    async fn get_returns_the_cached_response_on_a_hit() {
        let fixture = fixture();

        let first = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;
        let second = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;

        assert_eq!(first.expect("first call"), json!({"call": 1}));
        assert_eq!(second.expect("second call"), json!({"call": 1}));
        assert_eq!(calls(&fixture), 1);
    }

    #[tokio::test]
    async fn get_calls_the_inner_transport_on_a_miss() {
        let fixture = fixture();

        let _ = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;
        let _ = fixture.transport.get("/v1/catalog/us/songs/2", &[]).await;
        let _ = fixture
            .transport
            .get("/v1/catalog/us/songs/1", &[("l", "en")])
            .await;

        assert_eq!(calls(&fixture), 3);
    }

    #[tokio::test]
    async fn get_ignores_the_order_of_the_query() {
        let fixture = fixture();
        let path = "/v1/catalog/us/search";

        let _ = fixture.transport.get(path, &[("a", "1"), ("b", "2")]).await;
        let _ = fixture.transport.get(path, &[("b", "2"), ("a", "1")]).await;

        assert_eq!(calls(&fixture), 1);
    }

    #[tokio::test]
    async fn get_refreshes_the_response_after_its_expiry_time() {
        let fixture = fixture();
        let song = "/v1/catalog/us/songs/1";
        let playlist = "/v1/catalog/us/playlists/p.1";
        let search = "/v1/catalog/us/search";
        for path in [song, playlist, search] {
            let _ = fixture.transport.get(path, &[]).await;
        }

        fixture.clock.advance(TEN_MINUTES + Duration::from_secs(60));
        for path in [song, playlist, search] {
            let _ = fixture.transport.get(path, &[]).await;
        }
        assert_eq!(calls(&fixture), 4, "only search expired after 11 minutes");

        fixture.clock.advance(HOUR);
        for path in [song, playlist, search] {
            let _ = fixture.transport.get(path, &[]).await;
        }
        assert_eq!(calls(&fixture), 6, "playlist expired after 1 hour");

        fixture.clock.advance(SEVEN_DAYS);
        let _ = fixture.transport.get(song, &[]).await;
        assert_eq!(calls(&fixture), 7, "song expired after 7 days");
    }

    #[tokio::test]
    async fn get_passes_non_catalog_paths_through() {
        let fixture = fixture();

        let _ = fixture.transport.get("/v1/me/library/songs", &[]).await;
        let _ = fixture.transport.get("/v1/me/library/songs", &[]).await;

        assert_eq!(calls(&fixture), 2);
    }

    #[tokio::test]
    async fn clear_deletes_all_rows_and_returns_the_freed_bytes() {
        let fixture = fixture();
        let _ = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;
        let _ = fixture.transport.get("/v1/catalog/us/songs/2", &[]).await;

        assert_eq!(
            fixture.transport.used_bytes().await.expect("size is read"),
            20
        );
        let freed = fixture.transport.clear().await.expect("cache is cleared");

        assert_eq!(freed, 20);
        let _ = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;
        assert_eq!(calls(&fixture), 3);
    }

    #[tokio::test]
    async fn get_works_when_the_cache_fails() {
        let fixture = fixture();
        fixture
            .database
            .run(|connection| Ok(connection.execute_batch("DROP TABLE api_cache")?))
            .await
            .expect("table can be dropped");

        let value = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;

        assert_eq!(value.expect("inner answer is returned"), json!({"call": 1}));
    }

    #[tokio::test]
    async fn get_does_not_cache_an_inner_error() {
        struct Failing;

        #[async_trait::async_trait]
        impl ApiTransport for Failing {
            async fn get(&self, _path: &str, _query: &[(&str, &str)]) -> Result<Value> {
                Err(Error::Transport(TransportError::NotReady))
            }
        }
        let database = Database::open_in_memory().expect("database can be opened");
        let transport = CachingTransport::new(Failing, database.clone());

        let error = transport.get("/v1/catalog/us/songs/1", &[]).await;

        assert!(error.is_err());
        let rows = database
            .run(|connection| {
                Ok(
                    connection.query_row("SELECT count(*) FROM api_cache", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                )
            })
            .await
            .expect("rows can be counted");
        assert_eq!(rows, 0);
    }

    #[tokio::test]
    async fn get_evicts_the_rows_that_expire_first_above_the_byte_limit() {
        let fixture = fixture();
        // Each body is 10 bytes, so the limit keeps two rows.
        let transport = CachingTransport::with_clock(
            fixture.inner.clone(),
            fixture.database.clone(),
            fixture.clock.clone(),
        )
        .with_max_bytes(25);
        for id in ["a", "b", "c"] {
            let _ = transport
                .get(&format!("/v1/catalog/us/songs/{id}"), &[])
                .await;
            fixture.clock.advance(Duration::from_secs(1));
        }
        assert_eq!(calls(&fixture), 3);

        let _ = transport.get("/v1/catalog/us/songs/b", &[]).await;
        assert_eq!(calls(&fixture), 3, "newer rows are kept");
        let _ = transport.get("/v1/catalog/us/songs/a", &[]).await;
        assert_eq!(calls(&fixture), 4, "oldest row was evicted");
    }

    #[tokio::test]
    async fn write_deletes_expired_rows() {
        let fixture = fixture();
        let _ = fixture.transport.get("/v1/catalog/us/search", &[]).await;
        fixture.clock.advance(TEN_MINUTES + Duration::from_secs(1));

        let _ = fixture.transport.get("/v1/catalog/us/songs/1", &[]).await;

        assert_eq!(row_count(&fixture.database).await, 1);
    }

    #[tokio::test]
    async fn sweeper_deletes_expired_rows_without_requests() {
        let fixture = fixture();
        let _ = fixture.transport.get("/v1/catalog/us/search", &[]).await;
        fixture.clock.advance(TEN_MINUTES + Duration::from_secs(1));

        let sweeper = fixture.transport.spawn_sweeper(Duration::from_millis(10));
        for _ in 0..100 {
            if row_count(&fixture.database).await == 0 {
                break;
            }
            time::sleep(Duration::from_millis(10)).await;
        }
        sweeper.abort();

        assert_eq!(row_count(&fixture.database).await, 0);
    }

    async fn row_count(database: &Database) -> i64 {
        database
            .run(|connection| {
                Ok(
                    connection.query_row("SELECT count(*) FROM api_cache", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                )
            })
            .await
            .expect("rows can be counted")
    }

    #[test]
    fn expiry_for_follows_the_resource_type() {
        assert_eq!(expiry_for("/v1/catalog/us/albums/1"), Some(SEVEN_DAYS));
        assert_eq!(expiry_for("/v1/catalog/us/artists/1"), Some(SEVEN_DAYS));
        assert_eq!(expiry_for("/v1/catalog/us/playlists/1"), Some(HOUR));
        assert_eq!(expiry_for("/v1/catalog/us/search"), Some(TEN_MINUTES));
        assert_eq!(expiry_for("/v1/catalog/us/charts"), Some(HOUR));
        assert_eq!(expiry_for("/v1/me/library/songs"), None);
    }
}
