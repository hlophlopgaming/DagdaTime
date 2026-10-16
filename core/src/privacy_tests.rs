use super::*;
use serde_json::json;

#[test]
fn deletion_splits_intervals_flushes_tails_and_preserves_outside_history() {
    let mut db = Storage::open_in_memory().unwrap();
    db.heartbeat("window", &json!({"app":"editor"}), 100, 1000)
        .unwrap();
    db.heartbeat("window", &json!({"app":"editor"}), 400, 1000)
        .unwrap();
    db.delete_history(200, 300).unwrap();
    let events = db.events(None, 0, 500).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.start_ts, e.end_ts))
            .collect::<Vec<_>>(),
        vec![(100, 200), (300, 400)]
    );
    assert!(db.delete_history(300, 200).is_err());
}

#[test]
fn deleting_a_range_handles_all_overlap_shapes_tags_and_live_tails() {
    let mut db = Storage::open_in_memory().unwrap();
    for (bucket, start, end) in [
        ("spans", 100, 500),
        ("left", 100, 250),
        ("right", 250, 500),
        ("inside", 220, 280),
        ("before", 50, 200),
        ("after", 300, 600),
    ] {
        db.record_interval(bucket, &json!({"app":bucket}), start, end)
            .unwrap();
    }
    db.tag_interval("Work", "retain this note", 100, 500, false)
        .unwrap();
    for ts in [100, 200, 300, 400] {
        let mut batch = db.buffered_batch(ts);
        batch
            .heartbeat("tail", &json!({"app":"buffered"}), ts, 1000)
            .unwrap();
        batch.commit().unwrap();
    }
    db.delete_history(200, 300).unwrap();
    assert!(db.events(None, 200, 300).unwrap().is_empty());
    let events = db.events(Some("tail"), 0, 1000).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.start_ts, e.end_ts))
            .collect::<Vec<_>>(),
        vec![(100, 200), (300, 400)]
    );
    let ranges = db.tag_ranges(0, 1000).unwrap();
    assert_eq!(
        ranges
            .iter()
            .map(|r| (r.start_ts, r.end_ts))
            .collect::<Vec<_>>(),
        vec![(100, 200), (300, 500)]
    );
    assert!(ranges
        .iter()
        .all(|r| r.note.as_deref() == Some("retain this note")));
    db.flush().unwrap();
    assert!(db.events(None, 200, 300).unwrap().is_empty());
}

#[test]
fn deleting_history_preserves_overlapping_tag_assignment_outside_the_range() {
    let mut db = Storage::open_in_memory().unwrap();
    db.record_interval("afk", &json!({"status":"active"}), 100, 500)
        .unwrap();
    db.record_interval("window", &json!({"app":"editor"}), 100, 500)
        .unwrap();
    let older = db
        .put(Collection::Tags, None, json!({"name":"Older"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let newer = db
        .put(Collection::Tags, None, json!({"name":"Newer"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":older,"start_ts":100,"end_ts":500}),
    )
    .unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":newer,"start_ts":350,"end_ts":500}),
    )
    .unwrap();
    let before = serde_json::to_value(db.report(350, 500).unwrap()).unwrap();
    db.delete_history(200, 300).unwrap();
    assert_eq!(
        serde_json::to_value(db.report(350, 500).unwrap()).unwrap(),
        before
    );
    let archive = db.export_archive(0, 1000).unwrap();
    let mut restored = Storage::open_in_memory().unwrap();
    restored
        .import_json(&serde_json::to_vec(&archive).unwrap())
        .unwrap();
    assert_eq!(
        serde_json::to_value(restored.report(350, 500).unwrap()).unwrap(),
        before
    );
}
