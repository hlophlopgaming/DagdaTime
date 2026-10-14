use super::*;
use serde_json::json;

#[test]
fn hierarchy_split_replace_and_undo_are_atomic() {
    let mut db = Storage::open_in_memory().unwrap();
    let undo = db
        .tag_interval("Client, Project", "meeting", 100, 500, false)
        .unwrap();
    let tags = db.list(Collection::Tags).unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[1]["parent_id"], tags[0]["id"]);
    let before = db.list(Collection::TagRanges).unwrap();
    let change = db.tag_interval("Other", "", 200, 300, false).unwrap();
    let ranges = db.list(Collection::TagRanges).unwrap();
    assert_eq!(ranges.len(), 3);
    assert_eq!(
        ranges
            .iter()
            .map(|v| v["end_ts"].as_i64().unwrap() - v["start_ts"].as_i64().unwrap())
            .sum::<i64>(),
        400
    );
    db.undo_tags(change).unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap(), before);
    let change = db.tag_interval("Other", "", 200, 300, true).unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap().len(), 1);
    db.undo_tags(change).unwrap();
    db.undo_tags(undo).unwrap();
    assert!(db.list(Collection::Tags).unwrap().is_empty());
}

#[test]
fn invalid_hierarchy_rolls_back_and_undo_preserves_unrelated_edits() {
    let mut db = Storage::open_in_memory().unwrap();
    assert!(db
        .tag_interval("Client, , Task", "", 0, 100, false)
        .is_err());
    assert!(db.list(Collection::Tags).unwrap().is_empty());
    let undo = db.tag_interval("Work", "", 0, 100, false).unwrap();
    db.put(Collection::Tags, None, json!({"name":"External"}))
        .unwrap();
    db.undo_tags(undo).unwrap();
    assert_eq!(db.list(Collection::Tags).unwrap()[0]["name"], "External");
}

#[test]
fn manager_merge_archive_delete_and_undo_preserve_history() {
    let mut db = Storage::open_in_memory().unwrap();
    db.tag_interval("A", "", 0, 100, false).unwrap();
    db.tag_interval("B", "", 100, 200, false).unwrap();
    let tags = db.list(Collection::Tags).unwrap();
    let a = tags[0]["id"].as_i64().unwrap();
    let b = tags[1]["id"].as_i64().unwrap();
    let undo = db.merge_tags(a, b).unwrap();
    assert!(db
        .list(Collection::TagRanges)
        .unwrap()
        .iter()
        .all(|r| r["tag_id"] == b));
    db.undo_tags(undo).unwrap();
    db.put(
        Collection::Tags,
        Some(a),
        json!({"name":"Renamed","archived":true}),
    )
    .unwrap();
    assert_eq!(db.get(Collection::Tags, a).unwrap()["archived"], 1);
    let undo = db.delete_tag_tree(a).unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap().len(), 1);
    db.undo_tags(undo).unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap().len(), 2);
}

#[test]
fn completed_afk_keeps_full_interval_across_midnight_and_health_changes() {
    let mut db = Storage::open_in_memory().unwrap();
    let state = |activity, recording| json!({"machine":"awake","activity":activity,"recording":recording,"locked":false});
    db.record_interval(
        "computer",
        &state("away", "recording"),
        86_000_000,
        86_400_000,
    )
    .unwrap();
    db.record_interval(
        "computer",
        &state("away", "unobserved"),
        86_400_000,
        87_000_000,
    )
    .unwrap();
    db.record_interval(
        "computer",
        &state("active", "recording"),
        87_000_000,
        87_001_000,
    )
    .unwrap();
    assert_eq!(db.completed_afk().unwrap(), Some((86_000_000, 87_000_000)));
    db.record_interval(
        "computer",
        &state("active", "recording"),
        87_001_000,
        87_002_000,
    )
    .unwrap();
    assert_eq!(db.completed_afk().unwrap(), Some((86_000_000, 87_000_000)));
}

#[test]
fn migration_from_v1_preserves_tag_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("old.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    connection.execute_batch("PRAGMA user_version=1; INSERT INTO tags(id,name) VALUES(1,'Old'); INSERT INTO tag_ranges(tag_id,start_ts,end_ts) VALUES(1,0,100);").unwrap();
    drop(connection);
    let db = Storage::open(path).unwrap();
    assert_eq!(db.list(Collection::Tags).unwrap()[0]["archived"], 0);
    assert_eq!(db.list(Collection::TagRanges).unwrap()[0]["end_ts"], 100);
}

#[test]
fn edit_move_resize_delete_and_failed_merge_are_reversible() {
    let mut db = Storage::open_in_memory().unwrap();
    db.tag_interval("Parent, Child", "note", 100, 200, false)
        .unwrap();
    let original = db.list(Collection::TagRanges).unwrap();
    let id = original[0]["id"].as_i64().unwrap();
    let undo = db
        .edit_tag_interval(Some(id), "Parent, Child", "note", 300, 450, false)
        .unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap()[0]["start_ts"], 300);
    db.undo_tags(undo).unwrap();
    let tags = db.list(Collection::Tags).unwrap();
    assert!(db
        .merge_tags(
            tags[0]["id"].as_i64().unwrap(),
            tags[1]["id"].as_i64().unwrap()
        )
        .is_err());
    assert_eq!(db.list(Collection::TagRanges).unwrap(), original);
    let undo = db.delete_tag_range(id).unwrap();
    assert!(db.list(Collection::TagRanges).unwrap().is_empty());
    db.undo_tags(undo).unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap(), original);
}

#[test]
fn computer_states_and_recording_gaps_never_become_away_prompts() {
    for data in [
        json!({"machine":"asleep","activity":"away","recording":"recording"}),
        json!({"machine":"off","activity":"away","recording":"recording"}),
        json!({"machine":"awake","locked":true,"activity":"away","recording":"recording"}),
        json!({"machine":"awake","activity":"away","recording":"paused"}),
        json!({"machine":"awake","activity":"away","recording":"unknown"}),
        json!({"machine":"awake","activity":"unknown","recording":"unobserved"}),
    ] {
        let mut db = Storage::open_in_memory().unwrap();
        db.record_interval("computer", &data, 0, 900_000).unwrap();
        db.record_interval(
            "computer",
            &json!({"machine":"awake","activity":"active","recording":"recording"}),
            900_000,
            901_000,
        )
        .unwrap();
        assert_eq!(db.completed_afk().unwrap(), None, "{data}");
    }
    let mut db = Storage::open_in_memory().unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"away","recording":"recording"}),
        0,
        900_000,
    )
    .unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"active","recording":"recording"}),
        900_001,
        901_000,
    )
    .unwrap();
    assert_eq!(db.completed_afk().unwrap(), None);
}

#[test]
fn subtree_deletion_and_merge_include_rules_and_children_and_undo_restores_ids() {
    let mut db = Storage::open_in_memory().unwrap();
    db.tag_interval("Root, Leaf", "", 0, 100, false).unwrap();
    let tags = db.list(Collection::Tags).unwrap();
    let root = tags[0]["id"].as_i64().unwrap();
    let leaf = tags[1]["id"].as_i64().unwrap();
    let rule = db
        .put(
            Collection::Rules,
            None,
            json!({"field":"app","pattern":"editor","tag_id":leaf}),
        )
        .unwrap();
    let undo = db.delete_tag_tree(root).unwrap();
    assert!(db.list(Collection::Tags).unwrap().is_empty());
    assert!(db.list(Collection::Rules).unwrap().is_empty());
    db.undo_tags(undo).unwrap();
    assert_eq!(
        db.get(Collection::Rules, rule["id"].as_i64().unwrap())
            .unwrap(),
        rule
    );
    let undo = db.merge_tags(leaf, root).unwrap();
    assert_eq!(db.list(Collection::Rules).unwrap()[0]["tag_id"], root);
    db.undo_tags(undo).unwrap();
    assert_eq!(db.list(Collection::Tags).unwrap(), tags);
}

#[test]
fn undo_retains_only_changed_rows_in_large_history() {
    let mut db = Storage::open_in_memory().unwrap();
    db.connection.execute_batch("INSERT INTO tags(id,name) VALUES(1,'Work'); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<5000) INSERT INTO tag_ranges(tag_id,start_ts,end_ts,note) SELECT 1,x*100,x*100+50,'history' FROM n;").unwrap();
    let undo = db.update_tag(1, "Renamed", None, false).unwrap();
    assert_eq!(undo.retained_rows(), 2);
    db.undo_tags(undo).unwrap();
    let undo = db.tag_interval("Work", "new", 10, 20, false).unwrap();
    assert_eq!(undo.retained_rows(), 1);
    db.undo_tags(undo).unwrap();
    assert_eq!(db.tag_ranges(0, 600_000).unwrap().len(), 5000);
}

#[test]
fn undo_conflict_is_atomic_and_preserves_new_dependents() {
    let mut db = Storage::open_in_memory().unwrap();
    let undo = db.tag_interval("Work", "", 0, 100, false).unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":1,"start_ts":200,"end_ts":300}),
    )
    .unwrap();
    assert!(db.undo_tags(undo).is_err());
    assert_eq!(db.tag_ranges(0, 400).unwrap().len(), 2);
    let undo = db.update_tag(1, "Rename", None, false).unwrap();
    db.put(Collection::Tags, Some(1), json!({"name":"External"}))
        .unwrap();
    assert!(db.undo_tags(undo).is_err());
    assert_eq!(db.tags().unwrap()[0].name, "External");
}

#[test]
fn split_undo_handles_gaps_in_range_ids() {
    let mut db = Storage::open_in_memory().unwrap();
    db.connection.execute_batch("INSERT INTO tags(id,name) VALUES(1,'Work'); INSERT INTO tag_ranges(id,tag_id,start_ts,end_ts) VALUES(1,1,0,10),(10,1,100,200);").unwrap();
    let before = db.tag_ranges(0, 300).unwrap();
    let undo = db.tag_interval("Work", "", 120, 180, false).unwrap();
    db.undo_tags(undo).unwrap();
    assert_eq!(db.tag_ranges(0, 300).unwrap(), before);
}
