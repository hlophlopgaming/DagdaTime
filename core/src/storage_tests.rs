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
    assert_eq!(version, 5);
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
    connection.pragma_update(None, "user_version", 6).unwrap();
    assert!(matches!(
        Storage::open(&path),
        Err(StorageError::UnsupportedSchema(6))
    ));
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6);
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

#[test]
fn buffered_ticks_are_visible_without_database_writes() {
    let mut storage = Storage::open_in_memory().unwrap();
    for ts in 0..30 {
        let mut batch = storage.buffered_batch(ts * 1000);
        batch
            .heartbeat(
                "window",
                &serde_json::json!({"app":"editor"}),
                ts * 1000,
                3000,
            )
            .unwrap();
        batch.commit().unwrap();
        if ts > 0 {
            assert_eq!(
                storage.events(Some("window"), 0, 60_000).unwrap()[0].end_ts,
                ts * 1000
            );
        }
    }
    assert_eq!(
        storage
            .connection
            .query_row("SELECT end_ts FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    let mut batch = storage.buffered_batch(30_000);
    batch
        .heartbeat("window", &serde_json::json!({"app":"editor"}), 30_000, 3000)
        .unwrap();
    batch.commit().unwrap();
    assert_eq!(
        storage
            .connection
            .query_row("SELECT end_ts FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        30_000
    );
}

#[test]
fn buffered_transition_flushes_all_buckets_and_drop_rolls_back() {
    let mut storage = Storage::open_in_memory().unwrap();
    for ts in [1000, 2000] {
        let mut batch = storage.buffered_batch(ts);
        batch
            .heartbeat("window", &serde_json::json!("a"), ts, 3000)
            .unwrap();
        batch
            .heartbeat("afk", &serde_json::json!("active"), ts, 3000)
            .unwrap();
        batch.commit().unwrap();
    }
    {
        let mut batch = storage.buffered_batch(3000);
        batch
            .heartbeat("window", &serde_json::json!("discard"), 3000, 3000)
            .unwrap();
    }
    let mut batch = storage.buffered_batch(3000);
    batch.extend_last("window", 3000, 3000).unwrap();
    batch
        .heartbeat("window", &serde_json::json!("b"), 3000, 3000)
        .unwrap();
    batch.commit().unwrap();
    let persisted: i64 = storage
        .connection
        .query_row("SELECT end_ts FROM events WHERE bucket='afk'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(persisted, 2000);
    assert_eq!(
        storage.events(Some("window"), 0, 5000).unwrap()[0].end_ts,
        3000
    );
    assert_eq!(
        storage
            .connection
            .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
}

#[test]
fn buffered_wal_is_at_least_twenty_times_smaller_for_stable_tracking() {
    fn simulate(buffered: bool) -> u64 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("activity.sqlite");
        let mut storage = Storage::open(&path).unwrap();
        storage
            .connection
            .execute_batch("PRAGMA wal_autocheckpoint=0; PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        for second in 0..=600 {
            let now = second * 1000;
            let data = serde_json::json!({"app":"editor","title":"Document"});
            if buffered {
                let mut batch = storage.buffered_batch(now);
                batch.extend_last("window", now, 3000).unwrap();
                batch.heartbeat("window", &data, now, 3000).unwrap();
                batch
                    .heartbeat("afk", &serde_json::json!({"status":"active"}), now, 3000)
                    .unwrap();
                batch.commit().unwrap();
            } else {
                let batch = storage.batch().unwrap();
                batch.extend_last("window", now, 3000).unwrap();
                batch.heartbeat("window", &data, now, 3000).unwrap();
                batch
                    .heartbeat("afk", &serde_json::json!({"status":"active"}), now, 3000)
                    .unwrap();
                batch.commit().unwrap();
            }
        }
        std::fs::metadata(path.with_file_name("activity.sqlite-wal"))
            .unwrap()
            .len()
    }
    let before = simulate(false);
    let after = simulate(true);
    eprintln!(
        "600 s stable tracking WAL: {before} -> {after} bytes ({:.1}x)",
        before as f64 / after as f64
    );
    assert!(before > after * 20);
}

#[test]
fn buffered_overlay_crosses_query_boundary_and_flush_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("activity.sqlite");
    let mut storage = Storage::open(&path).unwrap();
    for now in [1000, 2000, 3000] {
        let mut batch = storage.buffered_batch(now);
        batch
            .heartbeat("window", &serde_json::json!("a"), now, 3000)
            .unwrap();
        batch.commit().unwrap();
    }
    assert_eq!(storage.events(None, 2500, 4000).unwrap()[0].end_ts, 3000);
    storage.flush().unwrap();
    drop(storage);
    let storage = Storage::open(path).unwrap();
    assert_eq!(storage.events(None, 2500, 4000).unwrap()[0].end_ts, 3000);
}

#[test]
fn failed_buffered_flush_keeps_the_previous_overlay_for_retry() {
    let mut storage = Storage::open_in_memory().unwrap();
    for now in [1000, 2000] {
        let mut batch = storage.buffered_batch(now);
        batch
            .heartbeat("window", &serde_json::json!("a"), now, 3000)
            .unwrap();
        batch.commit().unwrap();
    }
    storage.connection.execute_batch("CREATE TRIGGER deny_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT, 'disk error'); END;").unwrap();
    assert!(storage.flush().is_err());
    assert_eq!(storage.events(None, 0, 5000).unwrap()[0].end_ts, 2000);
    storage
        .connection
        .execute_batch("DROP TRIGGER deny_update")
        .unwrap();
    storage.flush().unwrap();
    assert_eq!(
        storage
            .connection
            .query_row("SELECT end_ts FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2000
    );
}

#[test]
fn evidence_intervals_reject_overlap_and_keep_zero_length_markers() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .record_interval(
            "computer",
            &serde_json::json!({"machine":"asleep","source":"mock OS"}),
            1000,
            2000,
        )
        .unwrap();
    assert!(storage
        .record_interval(
            "computer",
            &serde_json::json!({"machine":"off"}),
            1500,
            3000
        )
        .is_err());
    assert!(storage
        .record_interval("computer", &serde_json::json!({}), -1, 0)
        .is_err());
    storage
        .record_interval(
            "computer",
            &serde_json::json!({"recording":"stopped"}),
            2000,
            2000,
        )
        .unwrap();
    assert_eq!(
        storage.last_event("computer").unwrap().unwrap().data["recording"],
        "stopped"
    );
    assert_eq!(storage.events(Some("computer"), 0, 3000).unwrap().len(), 2);
    assert_eq!(storage.first_event_start("computer").unwrap(), Some(1000));
}

#[test]
fn v2_migration_preserves_tag_history_and_adds_range_index() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("v2.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/002_tag_archive.sql"))
        .unwrap();
    connection.execute_batch("PRAGMA user_version=2; INSERT INTO tags(id,name) VALUES(1,'Kept'); INSERT INTO tag_ranges(id,tag_id,start_ts,end_ts,note) VALUES(1,1,10,30,'Kept note');").unwrap();
    drop(connection);
    let db = Storage::open(&path).unwrap();
    assert_eq!(db.tags().unwrap()[0].name, "Kept");
    assert_eq!(
        db.tag_ranges(20, 40).unwrap()[0].note.as_deref(),
        Some("Kept note")
    );
    let version: i64 = db
        .connection
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 5);
}
