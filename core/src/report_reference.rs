use super::*;

// Frozen pre-optimization sweep: keep independent of the production boundary representation.
fn baseline(
    events: &[Event],
    tags: &[Tag],
    ranges: &[TagRange],
    from: i64,
    to: i64,
    strings: &mut HashSet<Arc<str>>,
) -> Result<Report, StorageError> {
    if from < 0 || to <= from {
        return Err(invalid("expected 0 <= from < to"));
    }
    let mut report = Report {
        from,
        to,
        ..Default::default()
    };
    // Index sets permit overlapping tags without rescanning every range at every boundary.
    let mut changes: BTreeMap<i64, Vec<(bool, bool, usize)>> = BTreeMap::new();
    changes.entry(from).or_default();
    changes.entry(to).or_default();
    for (is_tag, start, end, index) in events
        .iter()
        .enumerate()
        .map(|(i, e)| (false, e.start_ts, e.end_ts, i))
        .chain(
            ranges
                .iter()
                .enumerate()
                .map(|(i, r)| (true, r.start_ts, r.end_ts, i)),
        )
    {
        let (start, end) = (start.max(from), end.min(to));
        if end > start {
            changes
                .entry(start)
                .or_default()
                .push((is_tag, true, index));
            changes.entry(end).or_default().push((is_tag, false, index));
        }
    }
    let first_computer = events
        .iter()
        .filter(|e| e.bucket == "computer")
        .map(|e| e.start_ts)
        .min();
    let roots: HashMap<_, _> = tags
        .iter()
        .map(|tag| {
            let mut root = tag;
            let mut seen = BTreeSet::new();
            while let Some(parent) = root
                .parent_id
                .and_then(|id| tags.iter().find(|t| t.id == id))
            {
                if !seen.insert(root.id) {
                    break;
                }
                root = parent;
            }
            (tag.id, (root.id, root.name.as_str()))
        })
        .collect();
    let mut buckets: HashMap<&str, BTreeSet<(i64, usize)>> = HashMap::new();
    let mut assigned = BTreeSet::new();
    let points: Vec<_> = changes.keys().copied().collect();
    for pair in points.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        for &(is_tag, add, i) in &changes[&start] {
            let set = if is_tag {
                &mut assigned
            } else {
                buckets.entry(&events[i].bucket).or_default()
            };
            let key = (if is_tag { ranges[i].id } else { events[i].id }, i);
            if add {
                set.insert(key);
            } else {
                set.remove(&key);
            }
        }
        let current = |name: &str| {
            buckets
                .get(name)
                .and_then(|s| s.last())
                .map(|&(_, i)| &events[i])
        };
        if !observed_active(
            current("computer"),
            current("afk"),
            first_computer.is_none_or(|t| end <= t),
        ) {
            continue;
        }
        let (tag_id, tag) = assigned
            .last()
            .and_then(|&(_, i)| roots.get(&ranges[i].tag_id))
            .map(|(id, name)| (Some(*id), *name))
            .unwrap_or((None, "Untagged"));
        let window = current("window");
        let web = current("web");
        let application = window
            .and_then(|w| {
                w.data["app_name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| w.data["app"].as_str())
            })
            .unwrap_or("Unknown application");
        let document = web
            .and_then(|e| e.data["url"].as_str())
            .or_else(|| window.and_then(|e| e.data["title"].as_str()))
            .unwrap_or("Unknown document");
        report.active_ms += end - start;
        if let Some(last) = report.intervals.last_mut().filter(|last| {
            last.end_ts == start
                && last.tag_id == tag_id
                && last.application.as_ref() == application
                && last.document.as_ref() == document
        }) {
            last.end_ts = end;
        } else {
            report.intervals.push(ReportInterval {
                start_ts: start,
                end_ts: end,
                tag_id,
                tag: intern(strings, tag),
                application: intern(strings, application),
                document: intern(strings, document),
            });
        }
    }
    Ok(report)
}

#[test]
fn optimized_sweep_matches_original_on_diverse_histories() {
    use serde_json::json;
    let tags: Vec<_> = (1..=13)
        .map(|id| Tag {
            id,
            name: format!("Tag {id}"),
            color: None,
            parent_id: (id > 4).then_some(1 + id % 4),
            archived: 0,
        })
        .collect();
    for seed in 0..48_i64 {
        let mut events = Vec::new();
        let mut ranges = Vec::new();
        for i in 0..73_i64 {
            let start = (i * 37 + seed * 19) % 601;
            let end = start + (i * 13 + seed * 7) % 97;
            let (bucket, data) = match (i + seed) % 6 {
                0 => (
                    "computer",
                    json!({"machine":"awake", "activity":"active",
                    "recording":"unobserved", "locked": i % 5 == 0}),
                ),
                1 => (
                    "computer",
                    json!({"machine":"asleep", "activity":"away", "recording":"recording"}),
                ),
                2 => ("afk", json!({"status":"active"})),
                3 => (
                    "window",
                    json!({"app":format!("app {}", i % 7), "title":format!("文档 {i}")}),
                ),
                4 => ("web", json!({"url":format!("https://example.test/{i}")})),
                _ => ("other", json!({})),
            };
            if seed % 3 != 0 || bucket != "computer" {
                events.push(Event {
                    id: (i * 11) % 73,
                    bucket: bucket.into(),
                    start_ts: start,
                    end_ts: end,
                    data,
                });
            }
            ranges.push(TagRange {
                precedence_id: None,
                id: (i * 17) % 73,
                tag_id: 1 + i % 15,
                start_ts: start,
                end_ts: end + 3,
                note: None,
            });
        }
        // Unsorted input, overlapping buckets, ties, missing tags, zero durations and clipped bounds.
        for (from, to) in [(0, 750), (83, 487), (607, 690), (900, 1000)] {
            let expected =
                baseline(&events, &tags, &ranges, from, to, &mut HashSet::new()).unwrap();
            let actual = Report::calculate(&events, &tags, &ranges, from, to).unwrap();
            assert_eq!(actual, expected, "seed={seed}, bounds={from}..{to}");
            for group in [GroupBy::Tag, GroupBy::Application, GroupBy::Document] {
                assert_eq!(actual.rows(group, None), expected.rows(group, None));
            }
        }
    }
}
