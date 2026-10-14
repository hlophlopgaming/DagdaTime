use super::*;
use serde_json::json;

#[test]
fn crud_round_trip_and_foreign_keys() {
    let mut storage = Storage::open_in_memory().unwrap();
    let tag = storage
        .put(
            Collection::Tags,
            None,
            json!({"name":"Project","color":"#123456"}),
        )
        .unwrap();
    let id = tag["id"].as_i64().unwrap();
    let range = storage
        .put(
            Collection::TagRanges,
            None,
            json!({"tag_id":id,"start_ts":10,"end_ts":20}),
        )
        .unwrap();
    let rule = storage
        .put(
            Collection::Rules,
            None,
            json!({"tag_id":id,"field":"app","pattern":"editor"}),
        )
        .unwrap();
    assert_eq!(storage.list(Collection::Tags).unwrap(), vec![tag]);
    assert_eq!(
        storage
            .put(Collection::Tags, Some(id), json!({"name":"Renamed"}))
            .unwrap()["name"],
        "Renamed"
    );
    assert!(storage.delete(Collection::Tags, id).is_err());
    storage
        .delete(Collection::Rules, rule["id"].as_i64().unwrap())
        .unwrap();
    storage
        .delete(Collection::TagRanges, range["id"].as_i64().unwrap())
        .unwrap();
    assert!(storage.delete(Collection::Tags, id).unwrap());
    assert!(!storage.delete(Collection::Tags, id).unwrap());
}

#[test]
fn invalid_ranges_and_tag_cycles_are_rejected() {
    let mut storage = Storage::open_in_memory().unwrap();
    let parent = storage
        .put(Collection::Tags, None, json!({"name":"Parent"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let child = storage
        .put(
            Collection::Tags,
            None,
            json!({"name":"Child","parent_id":parent}),
        )
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    assert!(storage
        .put(
            Collection::Tags,
            Some(parent),
            json!({"name":"Parent","parent_id":child})
        )
        .is_err());
    assert!(storage
        .put(
            Collection::TagRanges,
            None,
            json!({"tag_id":parent,"start_ts":20,"end_ts":10})
        )
        .is_err());
    assert!(storage
        .put(Collection::Tags, None, json!({"name":""}))
        .is_err());
    assert!(storage
        .put(
            Collection::Rules,
            None,
            json!({"tag_id":parent,"field":"bad","pattern":".*"})
        )
        .is_err());
}

#[test]
fn event_query_uses_half_open_ranges_and_optional_bucket() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .heartbeat("window", &json!({"app":"editor"}), 0, 10_000)
        .unwrap();
    storage
        .heartbeat("window", &json!({"app":"editor"}), 5_000, 10_000)
        .unwrap();
    assert_eq!(storage.events(None, 1_000, 2_000).unwrap().len(), 1);
    assert!(storage
        .events(Some("window"), 5_000, 6_000)
        .unwrap()
        .is_empty());
    assert!(storage.events(Some("afk"), 0, 10_000).unwrap().is_empty());
    assert!(storage.events(None, 2_000, 1_000).is_err());
}

#[test]
fn event_query_includes_spanning_events_and_merges_buckets_in_time_order() {
    let mut storage = Storage::open_in_memory().unwrap();
    // window: [0,4000] app a, [4000,9000] app b, [12000,12000] app c
    for (ts, app) in [
        (0, "a"),
        (4_000, "a"),
        (4_000, "b"),
        (9_000, "b"),
        (12_000, "c"),
    ] {
        storage
            .heartbeat("window", &json!({ "app": app }), ts, 6_000)
            .unwrap();
    }
    // afk: one long event [1000,20000]
    for ts in [1_000, 20_000] {
        storage
            .heartbeat("afk", &json!({"status":"active"}), ts, 60_000)
            .unwrap();
    }
    // An event that started long before the range must still be returned.
    let spanning = storage.events(Some("afk"), 15_000, 16_000).unwrap();
    assert_eq!(spanning.len(), 1);
    assert_eq!((spanning[0].start_ts, spanning[0].end_ts), (1_000, 20_000));
    let all = storage.events(None, 3_000, 13_000).unwrap();
    let summary: Vec<_> = all
        .iter()
        .map(|event| (event.bucket.as_str(), event.start_ts))
        .collect();
    assert_eq!(
        summary,
        [
            ("window", 0),
            ("afk", 1_000),
            ("window", 4_000),
            ("window", 12_000)
        ]
    );
    // Half-open: an event ending exactly at `from` is excluded.
    assert!(storage
        .events(Some("window"), 9_000, 11_000)
        .unwrap()
        .is_empty());
    assert!(storage
        .events(Some("missing"), 0, 30_000)
        .unwrap()
        .is_empty());
}

#[test]
fn batch_commits_all_writes_or_none() {
    let mut storage = Storage::open_in_memory().unwrap();
    {
        let batch = storage.batch().unwrap();
        batch
            .heartbeat("window", &json!({"app":"a"}), 1_000, 3_000)
            .unwrap();
        batch
            .heartbeat("afk", &json!({"status":"active"}), 1_000, 3_000)
            .unwrap();
        // Dropped without commit: rolled back.
    }
    assert!(storage.events(None, 0, 10_000).unwrap().is_empty());
    let batch = storage.batch().unwrap();
    batch
        .heartbeat("window", &json!({"app":"a"}), 1_000, 3_000)
        .unwrap();
    batch.extend_last("window", 2_000, 3_000).unwrap();
    batch
        .heartbeat("afk", &json!({"status":"active"}), 2_000, 3_000)
        .unwrap();
    batch.commit().unwrap();
    let events = storage.events(None, 0, 10_000).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!((events[0].start_ts, events[0].end_ts), (1_000, 2_000));
}

#[test]
fn file_database_uses_write_ahead_log() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::open(directory.path().join("activity.sqlite")).unwrap();
    let mode: String = storage
        .connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn typed_tag_queries_keep_boundaries_recent_history_and_delete_totals() {
    let mut db = Storage::open_in_memory().unwrap();
    db.tag_interval("Parent, Child", "note", 10, 30, false)
        .unwrap();
    db.tag_interval("Later", "", 100, 200, false).unwrap();
    let tags = db.tags().unwrap();
    let parent = tags.iter().find(|t| t.name == "Parent").unwrap().id;
    let child = tags.iter().find(|t| t.name == "Child").unwrap().id;
    let ranges = db.tag_ranges(20, 100).unwrap();
    assert_eq!(ranges.len(), 1);
    assert_eq!((ranges[0].start_ts, ranges[0].end_ts), (10, 30));
    assert_eq!(ranges[0].note.as_deref(), Some("note"));
    assert!(db.tag_ranges(30, 100).unwrap().is_empty());
    assert!(db.tag_ranges(-1, 10).is_err());
    assert!(db.tag_ranges(10, 10).is_err());
    assert_eq!(db.recent_tag_ids().unwrap().len(), 2);
    assert_eq!(db.tag_tree_usage(parent).unwrap(), (1, 20));
    assert_eq!(db.tag_tree_usage(child).unwrap(), (1, 20));
    let cache: i64 = db
        .connection
        .query_row("PRAGMA cache_size", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cache, -512);
    let index: i64 = db
        .connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='idx_tag_ranges_start'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(index, 1);
    let plan: Vec<String> = db.connection.prepare("EXPLAIN QUERY PLAN SELECT id,tag_id,start_ts,end_ts,note FROM tag_ranges WHERE start_ts < ?2 AND end_ts > ?1 ORDER BY start_ts,id").unwrap().query_map(params![20,100], |r| r.get(3)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert!(plan
        .iter()
        .any(|step| step.contains("idx_tag_ranges_start")));
}
