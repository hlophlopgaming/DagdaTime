use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use draugr_time_core::{Event, Report, Tag, TagRange};
use serde_json::json;
use std::time::Duration;

fn fixture(count: usize, overlapping: bool, unique: bool) -> (Vec<Event>, Vec<Tag>, Vec<TagRange>) {
    let end = count as i64 * 1000;
    let mut events = vec![Event {
        id: 1,
        bucket: "computer".into(),
        start_ts: 0,
        end_ts: end,
        data: json!({"machine":"awake","activity":"active","recording":"recording"}),
    }];
    let tags = (1..=32)
        .map(|id| Tag {
            id,
            name: format!("Tag {id}"),
            color: None,
            parent_id: (id > 8).then_some(1 + id % 8),
            archived: 0,
        })
        .collect();
    let mut ranges = Vec::new();
    for i in 0..count {
        let start = i as i64 * 1000;
        events.push(Event {
            id: i as i64 + 2,
            bucket: "window".into(),
            start_ts: start,
            end_ts: (start + if overlapping { 3500 } else { 1000 }).min(end),
            data: json!({"app":format!("app-{}", i % 12),
                "title":format!("Document {}", if unique { i } else { i % 40 })}),
        });
        if i % 3 == 0 {
            ranges.push(TagRange {
                precedence_id: None,
                id: i as i64 + 1,
                tag_id: 1 + i as i64 % 32,
                start_ts: start + 100,
                end_ts: (start + 1700).min(end),
                note: None,
            });
        }
        if i % 7 == 0 {
            events.push(Event {
                id: count as i64 + i as i64 + 2,
                bucket: "web".into(),
                start_ts: start + 200,
                end_ts: (start + 800).min(end),
                data: json!({"url":format!("https://example.test/{}", i % 9)}),
            });
        }
    }
    (events, tags, ranges)
}

fn reports(c: &mut Criterion) {
    let mut group = c.benchmark_group("report_calculate");
    group
        .sample_size(30)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for count in [16, 5000, 25000] {
        for (shape, overlapping, unique) in [
            ("sequential", false, false),
            ("overlapping", true, false),
            ("unique", false, true),
        ] {
            let (events, tags, ranges) = fixture(count, overlapping, unique);
            let end = count as i64 * 1000;
            let report = Report::calculate(&events, &tags, &ranges, 0, end).unwrap();
            assert_eq!(report.active_ms, end);
            assert_eq!(
                report
                    .intervals
                    .iter()
                    .map(|i| i.end_ts - i.start_ts)
                    .sum::<i64>(),
                end
            );
            group.bench_function(BenchmarkId::new(shape, count), |b| {
                b.iter(|| {
                    black_box(
                        Report::calculate(
                            black_box(&events),
                            black_box(&tags),
                            black_box(&ranges),
                            0,
                            end,
                        )
                        .unwrap(),
                    )
                })
            });
        }
    }
    group.finish();
}
criterion_group!(benches, reports);
criterion_main!(benches);
