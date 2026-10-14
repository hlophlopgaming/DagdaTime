use super::*;
use serde_json::json;

fn rule(db: &mut Storage, field: &str, pattern: &str, tag: i64, priority: i64) -> i64 {
    db.put(
        Collection::Rules,
        None,
        json!({"field":field,"pattern":pattern,"tag_id":tag,"priority":priority}),
    )
    .unwrap()["id"]
        .as_i64()
        .unwrap()
}
fn fixture() -> Storage {
    let mut db = Storage::open_in_memory().unwrap();
    for name in ["Work", "Web", "Manual"] {
        db.put(Collection::Tags, None, json!({"name":name}))
            .unwrap();
    }
    db.record_interval(
        "window",
        &json!({"app":"editor","title":"Project Rust"}),
        0,
        1000,
    )
    .unwrap();
    db.record_interval(
        "web",
        &json!({"url":"https://example.org","title":"Reference"}),
        0,
        1000,
    )
    .unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"active","recording":"recording"}),
        0,
        700,
    )
    .unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"away","recording":"recording"}),
        700,
        1000,
    )
    .unwrap();
    db
}
#[test]
fn matching_priorities_clipping_manual_tags_preview_apply_and_undo() {
    let mut db = fixture();
    rule(&mut db, "app", "^editor$", 1, 1);
    let high = rule(&mut db, "url", "example\\.org", 2, 2);
    db.tag_interval("Manual", "keep", 200, 300, false).unwrap();
    let preview = db.preview_rules(100, 900, None).unwrap();
    assert_eq!(
        preview.iter().map(|r| r.end_ts - r.start_ts).sum::<i64>(),
        500
    );
    assert!(preview.iter().all(|r| r.tag_id == 2 && r.rule_id == high));
    let before = db.list(Collection::TagRanges).unwrap();
    let (applied, undo) = db.apply_rules(100, 900).unwrap();
    assert_eq!(applied, preview);
    let after = db.tag_ranges(0, 1000).unwrap();
    assert_eq!(
        after.iter().map(|r| r.end_ts - r.start_ts).sum::<i64>(),
        600
    );
    assert_eq!(db.preview_rules(100, 900, None).unwrap().len(), 0);
    db.undo_tags(undo).unwrap();
    assert_eq!(db.list(Collection::TagRanges).unwrap(), before);
}
#[test]
fn disabled_archived_ties_and_invalid_regex() {
    let mut db = fixture();
    let first = rule(&mut db, "title", "(?i)rust", 1, 4);
    rule(&mut db, "app", "editor", 2, 4);
    assert_eq!(db.preview_rules(0, 1000, None).unwrap()[0].rule_id, first);
    db.put(
        Collection::Rules,
        Some(first),
        json!({"field":"title","pattern":"rust","tag_id":1,"priority":4,"enabled":false}),
    )
    .unwrap();
    assert_eq!(db.preview_rules(0, 1000, None).unwrap()[0].tag_id, 2);
    db.put(
        Collection::Tags,
        Some(2),
        json!({"name":"Web","archived":true}),
    )
    .unwrap();
    assert!(db.preview_rules(0, 1000, None).unwrap().is_empty());
    assert!(db
        .put(
            Collection::Rules,
            None,
            json!({"field":"title","pattern":"[","tag_id":1})
        )
        .is_err());
}
#[test]
fn rule_draft_preview_does_not_write_and_unknown_states_do_not_invent_work() {
    let mut db = fixture();
    let draft = Rule {
        id: 0,
        field: "title".into(),
        pattern: "Rust".into(),
        tag_id: 1,
        priority: 0,
        enabled: true,
    };
    assert_eq!(
        db.preview_rules(0, 1000, Some(&draft)).unwrap()[0].end_ts,
        700
    );
    assert!(db.list(Collection::Rules).unwrap().is_empty());
    assert!(db.preview_rules(-1, 1000, Some(&draft)).is_err());
    db.record_interval(
        "window",
        &json!({"app":"editor","title":"Rust"}),
        1000,
        2000,
    )
    .unwrap();
    assert!(db
        .preview_rules(1000, 2000, Some(&draft))
        .unwrap()
        .is_empty());
}
#[test]
fn enabled_migration_preserves_old_rules() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let c = Connection::open(file.path()).unwrap();
    c.execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    c.execute_batch(include_str!("../migrations/002_tag_archive.sql"))
        .unwrap();
    c.execute_batch(include_str!("../migrations/003_tag_range_index.sql"))
        .unwrap();
    c.execute_batch("PRAGMA user_version=3; INSERT INTO tags(id,name) VALUES(1,'Keep'); INSERT INTO rules(id,field,pattern,tag_id,priority) VALUES(1,'app','editor',1,5)").unwrap();
    drop(c);
    let db = Storage::open(file.path()).unwrap();
    assert!(db.rules().unwrap()[0].enabled);
    assert_eq!(db.rules().unwrap()[0].priority, 5);
}

#[test]
fn history_undo_preserves_new_tracking_and_refuses_changed_applied_ranges() {
    let mut db = fixture();
    rule(&mut db, "app", "editor", 1, 0);
    let (_, undo) = db.apply_rules(0, 700).unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":3,"start_ts":1200,"end_ts":1400}),
    )
    .unwrap();
    db.undo_tags(undo).unwrap();
    assert_eq!(db.tag_ranges(0, 2000).unwrap().len(), 1);
    let (_, undo) = db.apply_rules(0, 700).unwrap();
    let r = db.tag_ranges(0, 700).unwrap()[0].clone();
    db.put(
        Collection::TagRanges,
        Some(r.id),
        json!({"tag_id":3,"start_ts":0,"end_ts":700}),
    )
    .unwrap();
    assert!(db.undo_tags(undo).is_err());
    assert_eq!(db.tag_ranges(0, 700).unwrap()[0].tag_id, 3);
}
#[test]
fn stale_confirmation_fails_without_partial_writes() {
    let mut db = fixture();
    rule(&mut db, "app", "editor", 1, 0);
    let preview = db.preview_rules(0, 700, None).unwrap();
    db.tag_interval("Manual", "", 200, 300, false).unwrap();
    assert!(db.apply_rules_checked(0, 700, &preview).is_err());
    assert_eq!(db.tag_ranges(0, 700).unwrap().len(), 1);
}

#[test]
fn invalid_draft_and_empty_or_large_history_are_safe() {
    let mut db = fixture();
    let mut draft = Rule {
        id: 0,
        field: "invalid".into(),
        pattern: "Rust".into(),
        tag_id: 1,
        priority: 0,
        enabled: true,
    };
    assert!(db.preview_rules(0, 1000, Some(&draft)).is_err());
    draft.field = "app".into();
    draft.pattern = "[".into();
    draft.enabled = false;
    assert!(db.preview_rules(0, 1000, Some(&draft)).is_err());
    rule(&mut db, "app", "editor", 1, 0);
    assert!(db.preview_rules(2000, 3000, None).unwrap().is_empty());
    for i in 0..5000 {
        let start = 2000 + i * 1000;
        db.record_interval(
            "window",
            &json!({"app":"editor","title":i.to_string()}),
            start,
            start + 1000,
        )
        .unwrap();
    }
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"active","recording":"recording"}),
        2000,
        5_002_000,
    )
    .unwrap();
    let preview = db.preview_rules(2000, 5_002_000, None).unwrap();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].end_ts - preview[0].start_ts, 5_000_000);
}
