use super::*;
use serde_json::json;
fn event(id: i64, bucket: &str, start: i64, end: i64, data: Value) -> Event {
    Event {
        id,
        bucket: bucket.into(),
        start_ts: start,
        end_ts: end,
        data,
    }
}
fn active(start: i64, end: i64) -> Event {
    event(
        1,
        "computer",
        start,
        end,
        json!({"machine":"awake","activity":"active","recording":"recording"}),
    )
}
#[test]
fn overview_partitions_hierarchy_overlaps_unknown_and_untagged() {
    let tags = vec![
        Tag {
            id: 1,
            name: "Client".into(),
            color: None,
            parent_id: None,
            archived: 0,
        },
        Tag {
            id: 2,
            name: "Task".into(),
            color: None,
            parent_id: Some(1),
            archived: 0,
        },
        Tag {
            id: 3,
            name: "Other client".into(),
            color: None,
            parent_id: None,
            archived: 0,
        },
    ];
    let ranges = vec![
        TagRange {
            precedence_id: None,
            id: 1,
            tag_id: 1,
            start_ts: 0,
            end_ts: 60,
            note: None,
        },
        TagRange {
            precedence_id: None,
            id: 2,
            tag_id: 2,
            start_ts: 10,
            end_ts: 50,
            note: None,
        },
        TagRange {
            precedence_id: None,
            id: 3,
            tag_id: 3,
            start_ts: 40,
            end_ts: 80,
            note: None,
        },
    ];
    let events = vec![
        active(0, 100),
        event(2, "window", 0, 70, json!({"app":"editor","title":"draft"})),
    ];
    let r = Report::calculate(&events, &tags, &ranges, 0, 100).unwrap();
    assert_eq!(r.active_ms, 100);
    assert_eq!(
        r.rows(GroupBy::Tag, None)
            .iter()
            .map(|r| r.duration_ms)
            .sum::<i64>(),
        100
    );
    assert_eq!(
        r.rows(GroupBy::Application, None)
            .iter()
            .map(|r| r.duration_ms)
            .sum::<i64>(),
        100
    );
    assert_eq!(
        r.rows(GroupBy::Tag, None)
            .iter()
            .find(|r| r.tag_id == Some(1))
            .unwrap()
            .duration_ms,
        40
    );
    assert_eq!(
        r.rows(GroupBy::Tag, None)
            .iter()
            .find(|r| r.tag_id == Some(3))
            .unwrap()
            .duration_ms,
        40
    );
    assert_eq!(
        r.rows(GroupBy::Tag, None)
            .iter()
            .find(|r| r.tag_id.is_none())
            .unwrap()
            .duration_ms,
        20
    );
    assert_eq!(
        r.rows(GroupBy::Application, None)
            .iter()
            .find(|r| r.label == "Unknown application")
            .unwrap()
            .duration_ms,
        30
    );
    assert!(
        (r.rows(GroupBy::Application, None)
            .iter()
            .map(|r| r.share)
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1e-10
    );
}
#[test]
fn inactive_states_gaps_and_diagnostics_do_not_invent_work() {
    let mut events = vec![active(0, 10), active(90, 100)];
    for (i, data) in [
        json!({"machine":"asleep","activity":"active","recording":"recording"}),
        json!({"machine":"off","activity":"active","recording":"recording"}),
        json!({"machine":"awake","activity":"away","recording":"recording"}),
        json!({"machine":"awake","activity":"active","recording":"paused"}),
        json!({"machine":"awake","activity":"active","recording":"stopped"}),
        json!({"machine":"awake","activity":"active","recording":"unknown"}),
        json!({"machine":"awake","activity":"unknown","recording":"unobserved"}),
        json!({"machine":"awake","activity":"active","recording":"recording","locked":true}),
    ]
    .into_iter()
    .enumerate()
    {
        events.push(event(
            i as i64 + 2,
            "computer",
            10 + i as i64 * 10,
            20 + i as i64 * 10,
            data,
        ));
    }
    assert_eq!(
        Report::calculate(&events, &[], &[], 0, 110)
            .unwrap()
            .active_ms,
        20
    );
    let events = vec![event(
        1,
        "computer",
        0,
        10,
        json!({"machine":"awake","activity":"active","recording":"unobserved","window_available":false,"reason":"backend failed"}),
    )];
    let r = Report::calculate(&events, &[], &[], 0, 20).unwrap();
    assert_eq!(r.active_ms, 10);
    assert_eq!(r.intervals[0].application.as_ref(), "Unknown application");
    assert_eq!(
        Report::calculate(&[], &[], &[], 0, 100).unwrap().active_ms,
        0
    );
    assert!(Report::calculate(&[], &[], &[], 100, 0).is_err());
}
#[test]
fn clipped_days_and_selection_totals_are_additive() {
    let e = vec![
        active(50, 250),
        event(2, "window", 50, 250, json!({"app":"app"})),
    ];
    let a = Report::calculate(&e, &[], &[], 0, 100).unwrap();
    let b = Report::calculate(&e, &[], &[], 100, 200).unwrap();
    let c = Report::calculate(&e, &[], &[], 200, 300).unwrap();
    assert_eq!(
        a.active_ms + b.active_ms + c.active_ms,
        Report::calculate(&e, &[], &[], 0, 300).unwrap().active_ms
    );
    assert_eq!(
        Report::calculate(&e, &[], &[], 75, 125).unwrap().active_ms,
        50
    );
}
#[test]
fn legacy_afk_stops_at_first_computer_evidence() {
    let e = vec![
        event(1, "afk", 0, 100, json!({"status":"active"})),
        active(50, 60),
    ];
    assert_eq!(
        Report::calculate(&e, &[], &[], 0, 100).unwrap().active_ms,
        60
    );
}
#[test]
fn five_thousand_events_keep_exact_totals() {
    let mut events = vec![active(0, 5000)];
    for i in 0..5000 {
        events.push(event(
            i + 2,
            "window",
            i,
            i + 1,
            json!({"app":format!("app{}",i%10)}),
        ));
    }
    let r = Report::calculate(&events, &[], &[], 0, 5000).unwrap();
    assert_eq!(r.active_ms, 5000);
    assert_eq!(r.rows(GroupBy::Application, None).len(), 10);
}
#[test]
fn local_archive_roundtrip_remaps_tags_and_keeps_reports() {
    let mut db = Storage::open_in_memory().unwrap();
    db.record_interval("computer", &active(0, 10).data, 0, 10)
        .unwrap();
    db.record_interval(
        "window",
        &json!({"app":"a","title":"line\n\"quoted\""}),
        0,
        10,
    )
    .unwrap();
    let tag = db
        .put(Collection::Tags, None, json!({"name":"Work"}))
        .unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":tag["id"],"start_ts":0,"end_ts":10}),
    )
    .unwrap();
    let archive = db.export_archive(0, 100).unwrap();
    let mut target = Storage::open_in_memory().unwrap();
    target
        .put(Collection::Tags, None, json!({"name":"existing"}))
        .unwrap();
    target
        .import_json(&serde_json::to_vec(&archive).unwrap())
        .unwrap();
    assert_eq!(
        db.report(0, 100).unwrap().rows(GroupBy::Tag, None)[0].duration_ms,
        target.report(0, 100).unwrap().rows(GroupBy::Tag, None)[0].duration_ms
    );
    assert_eq!(
        archive.events,
        target.export_archive(0, 100).unwrap().events
    );
    let csv = db.report(0, 100).unwrap().csv(GroupBy::Document, None);
    assert!(csv.contains("\"\"quoted\"\""));
}
#[test]
fn activitywatch_import_is_atomic_validated_and_preserves_nonoverlap() {
    let mut db = Storage::open_in_memory().unwrap();
    let payload = json!({"buckets":{"aw-watcher-window_host":{"type":"currentwindow","events":[{"timestamp":"2026-10-01T00:00:00Z","duration":1.25,"data":{"app":"editor","title":"work"}}]},"aw-watcher-afk_host":{"type":"afkstatus","events":[{"timestamp":"2026-10-01T00:00:00Z","duration":1.25,"data":{"status":"not-afk"}}]}}});
    db.import_json(&serde_json::to_vec(&payload).unwrap())
        .unwrap();
    let t = 1790812800000;
    assert_eq!(db.report(t, t + 2000).unwrap().active_ms, 1250);
    assert!(db
        .import_json(&serde_json::to_vec(&payload).unwrap())
        .is_err());
    assert_eq!(db.events(None, t, t + 2000).unwrap().len(), 2);
    let bad = json!({"buckets":{"x":{"type":"currentwindow","events":[{"timestamp":"bad","duration":-1,"data":{}}]}}});
    assert!(db.import_json(&serde_json::to_vec(&bad).unwrap()).is_err());
}
#[test]
fn archive_validation_and_mid_import_conflicts_leave_no_partial_records() {
    let mut source = Storage::open_in_memory().unwrap();
    source
        .record_interval("window", &json!({"app":"source"}), 10, 20)
        .unwrap();
    source
        .record_interval("window", &json!({"app":"source"}), 30, 40)
        .unwrap();
    source
        .put(Collection::Tags, None, json!({"name":"Imported"}))
        .unwrap();
    let archive = source.export_archive(0, 100).unwrap();
    let mut target = Storage::open_in_memory().unwrap();
    target
        .record_interval("window", &json!({"app":"existing"}), 35, 45)
        .unwrap();
    assert!(target
        .import_json(&serde_json::to_vec(&archive).unwrap())
        .is_err());
    assert_eq!(target.events(None, 0, 100).unwrap().len(), 1);
    assert!(target.tags().unwrap().is_empty());
    let mut invalid = archive;
    invalid.tags[0].parent_id = Some(invalid.tags[0].id);
    assert!(target
        .import_json(&serde_json::to_vec(&invalid).unwrap())
        .is_err());
    assert_eq!(target.events(None, 0, 100).unwrap().len(), 1);
}
#[test]
fn export_preserves_legacy_cutoff_even_when_computer_evidence_precedes_range() {
    let mut source = Storage::open_in_memory().unwrap();
    source
        .record_interval("afk", &json!({"status":"active"}), 0, 100)
        .unwrap();
    source
        .record_interval("computer", &json!({"recording":"unknown"}), 40, 45)
        .unwrap();
    let archive = source.export_archive(50, 100).unwrap();
    let mut target = Storage::open_in_memory().unwrap();
    target
        .import_json(&serde_json::to_vec(&archive).unwrap())
        .unwrap();
    assert_eq!(
        target.report(50, 100).unwrap().active_ms,
        source.report(50, 100).unwrap().active_ms
    );
}
#[test]
fn archive_roundtrip_preserves_overlap_precedence_and_boundary_markers() {
    let mut source = Storage::open_in_memory().unwrap();
    source
        .record_interval("computer", &active(0, 100).data, 0, 100)
        .unwrap();
    let a = source
        .put(Collection::Tags, None, json!({"name":"A"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let b = source
        .put(Collection::Tags, None, json!({"name":"B"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    source
        .put(
            Collection::TagRanges,
            None,
            json!({"tag_id":a,"start_ts":30,"end_ts":60}),
        )
        .unwrap();
    source
        .put(
            Collection::TagRanges,
            None,
            json!({"tag_id":b,"start_ts":10,"end_ts":90}),
        )
        .unwrap();
    source
        .record_interval("lifecycle", &json!({"kind":"resume"}), 20, 20)
        .unwrap();
    source
        .record_interval("lifecycle", &json!({"kind":"awake"}), 20, 30)
        .unwrap();
    let archive = source.export_archive(0, 100).unwrap();
    let mut target = Storage::open_in_memory().unwrap();
    target
        .import_json(&serde_json::to_vec(&archive).unwrap())
        .unwrap();
    let shape = |r: Report| {
        r.intervals
            .into_iter()
            .map(|i| (i.start_ts, i.end_ts, i.tag))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shape(target.report(0, 100).unwrap()),
        shape(source.report(0, 100).unwrap())
    );
    assert_eq!(target.events(Some("lifecycle"), 0, 100).unwrap().len(), 2);
}

#[test]
fn dense_report_exceeds_api_event_limit_without_losing_live_tails() {
    let mut db = Storage::open_in_memory().unwrap();
    db.connection.execute_batch("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<100000) INSERT INTO events(bucket,start_ts,end_ts,data) SELECT 'window',x*10,x*10+10,'{\"app\":\"editor\",\"title\":\"draft\"}' FROM n;").unwrap();
    db.record_interval("afk", &json!({"status":"active"}), 0, 1_000_020)
        .unwrap();
    db.record_interval(
        "window",
        &json!({"app":"terminal","title":"shell"}),
        1_000_010,
        1_000_020,
    )
    .unwrap();
    for timestamp in [1_000_020, 1_000_030] {
        let mut batch = db.buffered_batch(timestamp);
        batch
            .heartbeat("afk", &json!({"status":"active"}), timestamp, 3000)
            .unwrap();
        batch
            .heartbeat(
                "window",
                &json!({"app":"terminal","title":"shell"}),
                timestamp,
                3000,
            )
            .unwrap();
        batch.commit().unwrap();
    }
    let persisted: i64 = db
        .connection
        .query_row(
            "SELECT end_ts FROM events WHERE bucket='window' ORDER BY start_ts DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(persisted, 1_000_020);
    assert!(db.events(None, 0, 1_000_030).is_err());
    let report = db.report(0, 1_000_030).unwrap();
    assert_eq!(report.active_ms, 1_000_030);
    assert_eq!(report.intervals.len(), 2);
    assert_eq!(
        report.intervals.last().unwrap().application.as_ref(),
        "terminal"
    );
    let archive = db.export_archive(0, 1_000_030).unwrap();
    assert_eq!(archive.events.len(), 100_003);
    assert_eq!(
        archive
            .events
            .iter()
            .map(|e| e.id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        100_003
    );
    assert!(archive.events.iter().all(|e| e.end_ts <= 1_000_030));
}

#[test]
fn report_interns_repeated_labels_and_preserves_json_strings() {
    let events = [
        Event {
            id: 1,
            bucket: "afk".into(),
            start_ts: 0,
            end_ts: 30,
            data: json!({"status":"active"}),
        },
        Event {
            id: 2,
            bucket: "window".into(),
            start_ts: 0,
            end_ts: 10,
            data: json!({"app":"Editor","title":"draft"}),
        },
        Event {
            id: 3,
            bucket: "window".into(),
            start_ts: 10,
            end_ts: 20,
            data: json!({"app":"Terminal","title":"shell"}),
        },
        Event {
            id: 4,
            bucket: "window".into(),
            start_ts: 20,
            end_ts: 30,
            data: json!({"app":"Editor","title":"draft"}),
        },
    ];
    let report = Report::calculate(&events, &[], &[], 0, 30).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &report.intervals[0].application,
        &report.intervals[2].application
    ));
    assert!(std::sync::Arc::ptr_eq(
        &report.intervals[0].document,
        &report.intervals[2].document
    ));
    assert!(std::sync::Arc::ptr_eq(
        &report.intervals[0].tag,
        &report.intervals[2].tag
    ));
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["intervals"][0]["application"], "Editor");
}

#[test]
fn report_batch_boundaries_match_whole_calculation_with_overlap_and_legacy_cutoff() {
    let mut db = Storage::open_in_memory().unwrap();
    db.connection.execute_batch("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<17000) INSERT INTO events(bucket,start_ts,end_ts,data) SELECT 'window',x*10,x*10+10,CASE WHEN x%2=0 THEN '{\"app\":\"Editor\",\"title\":\"draft\"}' ELSE '{\"app\":\"Terminal\",\"title\":\"shell\"}' END FROM n;").unwrap();
    db.record_interval("afk", &json!({"status":"active"}), 0, 170_010)
        .unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"away","recording":"recording"}),
        160_000,
        170_010,
    )
    .unwrap();
    db.tag_interval("Work", "", 40_000, 120_000, false).unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":1,"start_ts":80_000,"end_ts":170_010}),
    )
    .unwrap();
    let mut events = db.events(None, 0, 170_010).unwrap();
    for e in &mut events {
        if e.bucket == "afk" {
            e.end_ts = e.end_ts.min(160_000);
        }
    }
    let expected = Report::calculate(
        &events,
        &db.tags().unwrap(),
        &db.tag_ranges(0, 170_010).unwrap(),
        0,
        170_010,
    )
    .unwrap();
    let actual = db.report(0, 170_010).unwrap();
    assert_eq!(actual, expected);
    assert!(std::sync::Arc::ptr_eq(
        &actual.intervals[0].application,
        &actual.intervals[16_000 - 2].application
    ));
}

#[test]
fn empty_report_handles_large_timestamp_ranges_without_daily_iteration() {
    let db = Storage::open_in_memory().unwrap();
    let report = db.report(0, i64::MAX).unwrap();
    assert_eq!(report.active_ms, 0);
    assert!(report.intervals.is_empty());
}

#[test]
fn report_context_fallbacks_and_web_precedence_remain_exact() {
    let events = vec![
        active(0, 40),
        event(
            2,
            "window",
            0,
            10,
            json!({"app":"editor","app_name":"Editor","title":"draft"}),
        ),
        event(
            3,
            "window",
            10,
            20,
            json!({"app":"terminal","app_name":"","title":"shell"}),
        ),
        event(4, "window", 20, 30, json!({"app":"browser","title":"page"})),
        event(5, "web", 20, 30, json!({"url":"https://example.test/文档"})),
    ];
    let report = Report::calculate(&events, &[], &[], 0, 40).unwrap();
    let contexts: Vec<_> = report
        .intervals
        .iter()
        .map(|i| {
            (
                i.start_ts,
                i.end_ts,
                i.application.as_ref(),
                i.document.as_ref(),
            )
        })
        .collect();
    assert_eq!(
        contexts,
        vec![
            (0, 10, "Editor", "draft"),
            (10, 20, "terminal", "shell"),
            (20, 30, "browser", "https://example.test/文档"),
            (30, 40, "Unknown application", "Unknown document"),
        ]
    );
}
