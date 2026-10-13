use super::*;
use serde_json::json;

fn events(storage: &Storage, bucket: &str) -> Vec<Event> {
    let mut query = storage
        .connection
        .prepare(
            "SELECT id, bucket, start_ts, end_ts, data FROM events WHERE bucket = ?1 ORDER BY id",
        )
        .unwrap();
    query
        .query_map([bucket], event_from_row)
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn identical_heartbeats_merge() {
    let mut storage = Storage::open_in_memory().unwrap();
    let data = json!({"app": "editor", "title": "main.rs"});
    let first = storage.heartbeat("window", &data, 1_000, 5_000).unwrap();
    let second = storage.heartbeat("window", &data, 2_000, 5_000).unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!((second.start_ts, second.end_ts), (1_000, 2_000));
    assert_eq!(events(&storage, "window"), vec![second]);
}

#[test]
fn window_change_splits_events() {
    let mut storage = Storage::open_in_memory().unwrap();
    let first = storage
        .heartbeat("window", &json!({"app": "editor"}), 1_000, 5_000)
        .unwrap();
    let second = storage
        .heartbeat("window", &json!({"app": "browser"}), 2_000, 5_000)
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(events(&storage, "window"), vec![first, second]);
}

#[test]
fn gap_greater_than_pulsetime_splits() {
    let mut storage = Storage::open_in_memory().unwrap();
    let data = json!({"app": "editor"});
    let first = storage.heartbeat("window", &data, 1_000, 5_000).unwrap();
    let second = storage.heartbeat("window", &data, 6_001, 5_000).unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!((second.start_ts, second.end_ts), (6_001, 6_001));
    assert_eq!(events(&storage, "window"), vec![first, second]);
}

#[test]
fn gap_equal_to_pulsetime_splits() {
    let mut storage = Storage::open_in_memory().unwrap();
    let data = json!({"app": "editor"});
    let first = storage.heartbeat("window", &data, 1_000, 5_000).unwrap();
    let second = storage.heartbeat("window", &data, 6_000, 5_000).unwrap();
    assert_ne!(first.id, second.id);
}

#[test]
fn merging_uses_last_end_and_keeps_buckets_independent() {
    let mut storage = Storage::open_in_memory().unwrap();
    let data = json!({"status": "active"});
    let first = storage.heartbeat("window", &data, 1_000, 2_000).unwrap();
    storage.heartbeat("afk", &data, 1_500, 2_000).unwrap();
    storage.heartbeat("window", &data, 2_000, 2_000).unwrap();
    let last = storage.heartbeat("window", &data, 3_000, 2_000).unwrap();
    assert_eq!(first.id, last.id);
    assert_eq!(last.end_ts, 3_000);
    assert_eq!(events(&storage, "afk").len(), 1);
}

#[test]
fn returning_to_previous_data_creates_new_event() {
    let mut storage = Storage::open_in_memory().unwrap();
    for (ts, app) in [(1_000, "editor"), (2_000, "browser"), (3_000, "editor")] {
        storage
            .heartbeat("window", &json!({"app": app}), ts, 5_000)
            .unwrap();
    }
    assert_eq!(events(&storage, "window").len(), 3);
}

#[test]
fn json_object_key_order_does_not_split_events() {
    let mut storage = Storage::open_in_memory().unwrap();
    let first_data = serde_json::from_str(r#"{"app":"editor","title":"main.rs"}"#).unwrap();
    let second_data = serde_json::from_str(r#"{"title":"main.rs","app":"editor"}"#).unwrap();
    let first = storage
        .heartbeat("window", &first_data, 1_000, 5_000)
        .unwrap();
    let second = storage
        .heartbeat("window", &second_data, 2_000, 5_000)
        .unwrap();
    assert_eq!(first.id, second.id);
}

#[test]
fn invalid_and_out_of_order_heartbeats_leave_database_unchanged() {
    let mut storage = Storage::open_in_memory().unwrap();
    let data = json!({"app": "editor"});
    let first = storage.heartbeat("window", &data, 2_000, 5_000).unwrap();
    for (bucket, ts, pulse) in [
        ("", 3_000, 5_000),
        ("window", -1, 5_000),
        ("window", 3_000, 0),
        ("window", 3_000, -1),
        ("window", 1_000, 5_000),
    ] {
        assert!(storage.heartbeat(bucket, &data, ts, pulse).is_err());
    }
    assert_eq!(events(&storage, "window"), vec![first]);
}

#[test]
fn duplicate_timestamp_does_not_extend_or_shrink_event() {
    let mut storage = Storage::open_in_memory().unwrap();
    let data = json!({"app": "editor"});
    let first = storage.heartbeat("window", &data, i64::MAX, 5_000).unwrap();
    let second = storage.heartbeat("window", &data, i64::MAX, 5_000).unwrap();
    assert_eq!(first, second);
}

#[test]
fn reopening_applies_migrations_once_and_preserves_events() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("activity.sqlite");
    let first = {
        let mut storage = Storage::open(&path).unwrap();
        storage
            .heartbeat("window", &json!({"app": "editor"}), 1_000, 5_000)
            .unwrap()
    };
    let mut reopened = Storage::open(&path).unwrap();
    assert_eq!(events(&reopened, "window"), vec![first.clone()]);
    let merged = reopened
        .heartbeat("window", &first.data, 2_000, 5_000)
        .unwrap();
    assert_eq!(first.id, merged.id);
    let version: i64 = reopened
        .connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    for table in ["events", "tags", "tag_ranges", "rules"] {
        reopened
            .connection
            .prepare(&format!("SELECT * FROM {table}"))
            .unwrap();
    }
    assert!(reopened
        .connection
        .execute(
            "INSERT INTO tag_ranges (tag_id, start_ts, end_ts) VALUES (999, 0, 1)",
            [],
        )
        .is_err());
}

#[test]
fn newer_schema_is_rejected_without_downgrading() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("future.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
    assert!(matches!(
        Storage::open(&path),
        Err(StorageError::UnsupportedSchema(2))
    ));
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
}

#[test]
fn failed_migration_rolls_back_schema_and_version() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("conflicting.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE TABLE tags (id INTEGER PRIMARY KEY);")
        .unwrap();
    assert!(Storage::open(&path).is_err());
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let events_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'events'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 0);
    assert_eq!(events_count, 0);
    connection.prepare("SELECT * FROM tags").unwrap();
}

#[test]
fn concurrent_writers_merge_into_one_event() {
    use std::sync::{Arc, Barrier};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("shared.sqlite");
    let barrier = Arc::new(Barrier::new(4));
    let mut threads = Vec::new();
    for _ in 0..4 {
        let mut storage = Storage::open(&path).unwrap();
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            storage
                .heartbeat("window", &json!({"app": "editor"}), 1_000, 5_000)
                .unwrap()
        }));
    }
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert!(results.iter().all(|event| event.id == results[0].id));
    let storage = Storage::open(&path).unwrap();
    assert_eq!(events(&storage, "window"), vec![results[0].clone()]);
}
