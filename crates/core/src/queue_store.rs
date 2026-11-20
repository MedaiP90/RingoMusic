// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Storage of the playback queue in the database.

use std::time::Duration;

use ringo_engine::ItemId;
use ringo_engine::protocol::RepeatMode;
use rusqlite::{Connection, OptionalExtension, params};

use crate::db::Database;
use crate::queue::{Queue, QueueItem};
use crate::{Error, Result};

/// Saves and loads the queue, the shuffle order, the repeat mode, and the
/// playback position. The queue survives a restart of the application.
#[derive(Clone, Debug)]
pub struct QueueStore {
    database: Database,
}

impl QueueStore {
    /// Creates a store that uses `database`.
    #[must_use]
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    /// Replaces the saved queue with `queue` and `position`, in one
    /// transaction. An empty queue clears the saved queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the database call fails.
    pub async fn save(&self, queue: &Queue, position: Duration) -> Result<()> {
        let queue = queue.clone();
        self.database
            .run(move |connection| write_queue(connection, &queue, position))
            .await
    }

    /// Saves the position only. It does nothing if no queue is saved.
    ///
    /// # Errors
    ///
    /// Returns an error if the database call fails.
    pub async fn save_position(&self, position: Duration) -> Result<()> {
        self.database
            .run(move |connection| {
                connection.execute(
                    "UPDATE queue_state SET position_ms = ?1",
                    [millis(position)],
                )?;
                Ok(())
            })
            .await
    }

    /// Loads the saved queue and position. Returns `None` if no queue is saved.
    ///
    /// # Errors
    ///
    /// Returns an error if the database call fails, or if the saved data is
    /// not a valid queue.
    pub async fn load(&self) -> Result<Option<(Queue, Duration)>> {
        self.database.run(|connection| read_queue(connection)).await
    }
}

fn write_queue(connection: &mut Connection, queue: &Queue, position: Duration) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "DELETE FROM queue_item; DELETE FROM queue_shuffle; DELETE FROM queue_history;
         DELETE FROM queue_state",
    )?;
    if let Some(current) = queue.current_index() {
        let mut insert_item =
            transaction.prepare("INSERT INTO queue_item VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
        for (place, item) in queue.items().iter().enumerate() {
            insert_item.execute(params![
                db_int(place),
                item.id.as_str(),
                item.title,
                item.artist,
                item.album,
                millis(item.duration),
                item.artwork_template,
            ])?;
        }
        if let Some(order) = queue.shuffle_order() {
            let mut insert_shuffle =
                transaction.prepare("INSERT INTO queue_shuffle VALUES (?1, ?2)")?;
            for (place, index) in order.iter().enumerate() {
                insert_shuffle.execute(params![db_int(place), db_int(*index)])?;
            }
        }
        let mut insert_history =
            transaction.prepare("INSERT INTO queue_history VALUES (?1, ?2)")?;
        for (place, id) in queue.history().enumerate() {
            insert_history.execute(params![db_int(place), id.as_str()])?;
        }
        transaction.execute(
            "INSERT INTO queue_state VALUES (1, ?1, ?2, ?3)",
            params![
                db_int(current),
                repeat_name(queue.repeat()),
                millis(position)
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn read_queue(connection: &Connection) -> Result<Option<(Queue, Duration)>> {
    let state = connection
        .query_row(
            "SELECT current_index, repeat, position_ms FROM queue_state",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((current, repeat, position_ms)) = state else {
        return Ok(None);
    };
    let items = read_items(connection)?;
    let shuffle = read_shuffle(connection)?;
    let repeat = parse_repeat(&repeat)?;
    let history = read_history(connection)?;
    let queue = Queue::from_parts(items, Some(db_index(current)), shuffle, repeat, history).ok_or(
        Error::InvalidQueue {
            reason: "the items, the current index, or the shuffle order do not fit",
        },
    )?;
    let position = from_millis(position_ms);
    Ok(Some((queue, position)))
}

fn read_items(connection: &Connection) -> Result<Vec<QueueItem>> {
    let mut statement = connection.prepare(
        "SELECT id, title, artist, album, duration_ms, artwork_template
         FROM queue_item ORDER BY place",
    )?;
    let items = statement
        .query_map([], |row| {
            Ok(QueueItem {
                id: ItemId::new(row.get::<_, String>(0)?),
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                duration: from_millis(row.get(4)?),
                artwork_template: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

fn read_history(connection: &Connection) -> Result<Vec<ItemId>> {
    let mut statement = connection.prepare("SELECT id FROM queue_history ORDER BY place")?;
    let history = statement
        .query_map([], |row| row.get::<_, String>(0).map(ItemId::new))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(history)
}

/// Reads the shuffle order. Returns `None` when shuffle was off.
fn read_shuffle(connection: &Connection) -> Result<Option<Vec<usize>>> {
    let mut statement =
        connection.prepare("SELECT item_index FROM queue_shuffle ORDER BY place")?;
    let order = statement
        .query_map([], |row| row.get::<_, i64>(0).map(db_index))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // ponytail: shuffle on with an empty order cannot be saved, because an empty queue is not saved.
    Ok((!order.is_empty()).then_some(order))
}

fn db_int(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Converts a stored number to an index. A negative number gives an index that
/// no queue has, so the queue check rejects it.
fn db_index(value: i64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn from_millis(value: i64) -> Duration {
    Duration::from_millis(u64::try_from(value).unwrap_or_default())
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn repeat_name(mode: RepeatMode) -> &'static str {
    match mode {
        RepeatMode::None => "none",
        RepeatMode::One => "one",
        RepeatMode::All => "all",
    }
}

fn parse_repeat(name: &str) -> Result<RepeatMode> {
    match name {
        "none" => Ok(RepeatMode::None),
        "one" => Ok(RepeatMode::One),
        "all" => Ok(RepeatMode::All),
        _ => Err(Error::InvalidQueue {
            reason: "unknown repeat mode",
        }),
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    fn item(id: &str) -> QueueItem {
        QueueItem {
            id: ItemId::new(id),
            title: format!("title {id}"),
            artist: "artist".to_owned(),
            album: "album".to_owned(),
            duration: Duration::from_millis(183_500),
            artwork_template: Some(format!("https://art/{id}/{{w}}x{{h}}.{{f}}")),
        }
    }

    fn queue_of(count: usize, start: usize) -> Queue {
        let items = (0..count)
            .map(|number| item(&format!("s{number}")))
            .collect();
        let mut queue = Queue::new();
        let _ = queue.replace(items, start);
        queue
    }

    fn store() -> QueueStore {
        QueueStore::new(Database::open_in_memory().expect("database can be opened"))
    }

    #[tokio::test]
    async fn load_returns_none_when_nothing_is_saved() {
        assert_eq!(store().load().await.expect("load works"), None);
    }

    #[tokio::test]
    async fn load_returns_the_saved_queue_without_shuffle() {
        let store = store();
        let mut queue = queue_of(4, 2);
        queue.set_repeat(RepeatMode::All);
        let position = Duration::from_millis(42_123);

        store.save(&queue, position).await.expect("save works");

        let (loaded, loaded_position) = store.load().await.expect("load works").expect("saved");
        assert_eq!(loaded, queue);
        assert_eq!(loaded_position, position);
    }

    #[tokio::test]
    async fn load_returns_the_saved_queue_with_shuffle() {
        let store = store();
        let mut queue = queue_of(6, 3);
        let _ = queue.set_shuffle(true, &mut StdRng::seed_from_u64(7));
        queue.set_repeat(RepeatMode::One);

        store
            .save(&queue, Duration::ZERO)
            .await
            .expect("save works");

        let (loaded, _) = store.load().await.expect("load works").expect("saved");
        assert_eq!(loaded, queue);
        assert_eq!(loaded.playback_order(), queue.playback_order());
        assert!(loaded.shuffle_enabled());
    }

    #[tokio::test]
    async fn load_returns_the_history() {
        let store = store();
        let mut queue = queue_of(3, 0);
        let _ = queue.advance();
        let _ = queue.advance();

        store
            .save(&queue, Duration::ZERO)
            .await
            .expect("save works");

        let (loaded, _) = store.load().await.expect("load works").expect("saved");
        assert_eq!(
            loaded.history().collect::<Vec<_>>(),
            queue.history().collect::<Vec<_>>()
        );
        assert_eq!(loaded.history().count(), 2);
    }

    #[tokio::test]
    async fn save_replaces_the_earlier_queue() {
        let store = store();
        store
            .save(&queue_of(5, 0), Duration::ZERO)
            .await
            .expect("save works");
        let second = queue_of(2, 1);

        store
            .save(&second, Duration::ZERO)
            .await
            .expect("save works");

        let (loaded, _) = store.load().await.expect("load works").expect("saved");
        assert_eq!(loaded, second);
    }

    #[tokio::test]
    async fn save_of_an_empty_queue_clears_the_saved_queue() {
        let store = store();
        store
            .save(&queue_of(3, 0), Duration::ZERO)
            .await
            .expect("save works");

        store
            .save(&Queue::new(), Duration::ZERO)
            .await
            .expect("save works");

        assert_eq!(store.load().await.expect("load works"), None);
    }

    #[tokio::test]
    async fn save_position_changes_only_the_position() {
        let store = store();
        let queue = queue_of(3, 1);
        store
            .save(&queue, Duration::ZERO)
            .await
            .expect("save works");

        store
            .save_position(Duration::from_secs(9))
            .await
            .expect("save works");

        let (loaded, position) = store.load().await.expect("load works").expect("saved");
        assert_eq!(loaded, queue);
        assert_eq!(position, Duration::from_secs(9));
    }

    #[tokio::test]
    async fn load_returns_an_error_for_a_current_index_outside_the_items() {
        let store = store();
        store
            .save(&queue_of(2, 0), Duration::ZERO)
            .await
            .expect("save works");
        store
            .database
            .run(|connection| {
                connection.execute("UPDATE queue_state SET current_index = 9", [])?;
                Ok(())
            })
            .await
            .expect("update works");

        let error = store.load().await.expect_err("the index is not valid");

        assert!(matches!(error, Error::InvalidQueue { .. }));
    }
}
