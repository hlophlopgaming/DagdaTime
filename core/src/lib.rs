//! Platform-independent storage and tracking logic for DagdaTime.

use std::{path::Path, time::Duration};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

/// An activity interval. Timestamps are Unix milliseconds in UTC.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub id: i64,
    pub bucket: String,
    pub start_ts: i64,
    pub end_ts: i64,
    pub data: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("database schema version {0} is newer than this application supports")]
    UnsupportedSchema(i64),
    #[error(
        "bucket must not be empty, timestamp must be nonnegative, and pulsetime must be positive"
    )]
    InvalidHeartbeat,
    #[error("heartbeat timestamp precedes the last event in its bucket")]
    OutOfOrderHeartbeat,
    #[error("invalid record: {0}")]
    InvalidRecord(String),
    #[error("record not found")]
    NotFound,
}

/// Local SQLite storage. Opening a database applies pending migrations atomically.
pub struct Storage {
    connection: Connection,
}

impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::initialize(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(mut connection: Connection) -> Result<Self, StorageError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        // The tracker writes every second. WAL appends to a log instead of rewriting
        // pages through a rollback journal, and NORMAL syncs at checkpoints rather
        // than on every commit: still crash-safe for the application, ~10x less I/O.
        // In-memory databases report "memory" and keep working unchanged.
        connection
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        match version {
            0 => {
                transaction.execute_batch(include_str!("../migrations/001_initial.sql"))?;
                transaction.pragma_update(None, "user_version", 1)?;
            }
            1 => {}
            other => return Err(StorageError::UnsupportedSchema(other)),
        }
        transaction.commit()?;
        Ok(Self { connection })
    }

    /// Record a heartbeat, merging only with the latest event in this bucket.
    /// `ts` and `pulsetime` are milliseconds. New events have zero duration;
    /// subsequent matching heartbeats extend them to their observed timestamp.
    /// Timestamps must be nondecreasing within each bucket.
    pub fn heartbeat(
        &mut self,
        bucket: &str,
        data: &Value,
        ts: i64,
        pulsetime: i64,
    ) -> Result<Event, StorageError> {
        let batch = self.batch()?;
        let event = batch.heartbeat(bucket, data, ts, pulsetime)?;
        batch.commit()?;
        Ok(event)
    }

    /// Start a group of writes that is committed atomically with one disk sync.
    /// Dropping the batch without calling [`Batch::commit`] rolls it back.
    pub fn batch(&mut self) -> Result<Batch<'_>, StorageError> {
        Ok(Batch {
            transaction: self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?,
        })
    }
}

/// Several event writes in one immediate transaction. The tracker records a whole
/// sampling tick through one batch instead of one transaction per write.
pub struct Batch<'a> {
    transaction: rusqlite::Transaction<'a>,
}

impl Batch<'_> {
    /// Same semantics as [`Storage::heartbeat`], inside this batch.
    pub fn heartbeat(
        &self,
        bucket: &str,
        data: &Value,
        ts: i64,
        pulsetime: i64,
    ) -> Result<Event, StorageError> {
        heartbeat_in(&self.transaction, bucket, data, ts, pulsetime)
    }

    /// Same semantics as [`Storage::extend_last`], inside this batch.
    pub fn extend_last(&self, bucket: &str, ts: i64, max_gap: i64) -> Result<(), StorageError> {
        records::extend_last_in(&self.transaction, bucket, ts, max_gap)
    }

    pub fn commit(self) -> Result<(), StorageError> {
        self.transaction.commit()?;
        Ok(())
    }
}

fn heartbeat_in(
    transaction: &Connection,
    bucket: &str,
    data: &Value,
    ts: i64,
    pulsetime: i64,
) -> Result<Event, StorageError> {
    if bucket.is_empty() || ts < 0 || pulsetime <= 0 {
        return Err(StorageError::InvalidHeartbeat);
    }
    let last = transaction
        .query_row(
            "SELECT id, bucket, start_ts, end_ts, data FROM events
             WHERE bucket = ?1 ORDER BY start_ts DESC, id DESC LIMIT 1",
            [bucket],
            event_from_row,
        )
        .optional()?;

    if let Some(mut event) = last {
        if ts < event.end_ts {
            return Err(StorageError::OutOfOrderHeartbeat);
        }
        if event.data == *data && ts - event.end_ts < pulsetime {
            transaction.execute(
                "UPDATE events SET end_ts = ?1 WHERE id = ?2",
                params![ts, event.id],
            )?;
            event.end_ts = ts;
            return Ok(event);
        }
    }

    transaction.execute(
        "INSERT INTO events (bucket, start_ts, end_ts, data) VALUES (?1, ?2, ?2, ?3)",
        params![bucket, ts, serde_json::to_string(data)?],
    )?;
    Ok(Event {
        id: transaction.last_insert_rowid(),
        bucket: bucket.to_owned(),
        start_ts: ts,
        end_ts: ts,
        data: data.clone(),
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let data: String = row.get(4)?;
    let data = serde_json::from_str(&data).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(Event {
        id: row.get(0)?,
        bucket: row.get(1)?,
        start_ts: row.get(2)?,
        end_ts: row.get(3)?,
        data,
    })
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;

#[cfg(test)]
mod crud_tests;

mod records;
pub use records::Collection;
