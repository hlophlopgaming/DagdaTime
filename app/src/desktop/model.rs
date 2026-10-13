use chrono::{Datelike, Local, NaiveDate, TimeZone};
use draugr_time_core::Event;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: i64,
    pub end: i64,
}
impl Range {
    pub const fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }
    pub fn duration(self) -> i64 {
        (self.end - self.start).max(0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub range: Range,
    pub key: String,
    pub label: String,
    pub detail: String,
    pub app: Option<String>,
    pub app_name: Option<String>,
    pub color: [f32; 3],
}
impl Segment {
    pub fn new(range: Range, key: &str, label: &str) -> Self {
        Self {
            range,
            key: key.into(),
            label: label.into(),
            detail: String::new(),
            app: None,
            app_name: None,
            color: color(key),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: &'static str,
    pub segments: Vec<Segment>,
}
#[derive(Clone, Debug)]
pub struct Summary {
    pub key: String,
    pub label: String,
    pub duration: i64,
    pub share: f64,
    pub color: [f32; 3],
    pub app: Option<String>,
    pub app_name: Option<String>,
}

pub fn clip(value: Range, bounds: Range) -> Option<Range> {
    let range = Range::new(value.start.max(bounds.start), value.end.min(bounds.end));
    (range.end > range.start).then_some(range)
}
fn merged(values: &[Range], bounds: Range) -> Vec<Range> {
    let mut values: Vec<_> = values
        .iter()
        .filter_map(|value| clip(*value, bounds))
        .collect();
    values.sort_by_key(|value| (value.start, value.end));
    let mut result: Vec<Range> = Vec::new();
    for value in values {
        if let Some(last) = result.last_mut().filter(|last| last.end >= value.start) {
            last.end = last.end.max(value.end);
        } else {
            result.push(value);
        }
    }
    result
}
pub fn union_duration(values: &[Range], bounds: Range) -> i64 {
    merged(values, bounds)
        .iter()
        .map(|value| value.duration())
        .sum()
}
pub fn uncovered(base: &[Range], covers: &[Range], bounds: Range) -> Vec<Range> {
    let mut result = Vec::new();
    let covers = merged(covers, bounds);
    for range in merged(base, bounds) {
        let mut cursor = range.start;
        for cover in &covers {
            if cover.end <= cursor {
                continue;
            }
            if cover.start >= range.end {
                break;
            }
            if cover.start > cursor {
                result.push(Range::new(cursor, cover.start.min(range.end)));
            }
            cursor = cursor.max(cover.end);
            if cursor >= range.end {
                break;
            }
        }
        if cursor < range.end {
            result.push(Range::new(cursor, range.end));
        }
    }
    result
}
pub fn summarize(segments: &[Segment], range: Range) -> Vec<Summary> {
    let mut groups: HashMap<&str, Vec<&Segment>> = HashMap::new();
    for segment in segments {
        if clip(segment.range, range).is_some() {
            groups.entry(&segment.key).or_default().push(segment);
        }
    }
    let mut result: Vec<_> = groups
        .into_iter()
        .map(|(key, group)| {
            let duration =
                union_duration(&group.iter().map(|s| s.range).collect::<Vec<_>>(), range);
            Summary {
                key: key.into(),
                label: group[0].label.clone(),
                duration,
                share: duration as f64 / range.duration().max(1) as f64 * 100.0,
                color: group[0].color,
                app: group[0].app.clone(),
                app_name: group[0].app_name.clone(),
            }
        })
        .collect();
    result.sort_by(|a, b| {
        b.duration
            .cmp(&a.duration)
            .then_with(|| a.label.cmp(&b.label))
    });
    result
}
pub fn pixel_range(view: Range, width: f32, a: f32, b: f32) -> Range {
    let time = |x: f32| {
        view.start
            + (x.clamp(0.0, width.max(1.0)) as f64 / width.max(1.0) as f64 * view.duration() as f64)
                .round() as i64
    };
    Range::new(time(a).min(time(b)), time(a).max(time(b)))
}
pub fn zoom(day: Range, view: Range, factor: f64, anchor: Option<i64>) -> Range {
    let span = ((view.duration() as f64 * factor).round() as i64).clamp(60_000, day.duration());
    let anchor = anchor.unwrap_or(view.start + view.duration() / 2);
    let position = (anchor - view.start) as f64 / view.duration().max(1) as f64;
    let start = (anchor - (span as f64 * position).round() as i64).clamp(day.start, day.end - span);
    Range::new(start, start + span)
}
pub fn continuous_span(segment: &Segment, values: &[Segment]) -> Range {
    let mut range = segment.range;
    for value in values.iter().rev() {
        if value.key == segment.key && value.range.end == range.start {
            range.start = value.range.start;
        }
    }
    for value in values {
        if value.key == segment.key
            && value.range.start <= range.end
            && value.range.end >= range.start
        {
            range.end = range.end.max(value.range.end);
        }
    }
    range
}
pub fn process_label(app: &str) -> &str {
    app.rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(app)
}
pub fn application_identity(process: &str, app_name: &str) -> String {
    format!("{process}\0{app_name}")
}
pub fn color(app: &str) -> [f32; 3] {
    let hash = app.chars().fold(2166136261u32, |hash, c| {
        (hash ^ c as u32).wrapping_mul(16777619)
    });
    let h = (hash % 360) as f32 / 60.0;
    let chroma = (1.0 - (2.0f32 * 0.56 - 1.0).abs()) * 0.48;
    let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
    let m = 0.56 - chroma / 2.0;
    let rgb = match h as u32 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    rgb.map(|c| c + m)
}
pub fn day_bounds(date: NaiveDate) -> Range {
    let midnight = |date: NaiveDate| {
        Local
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
            .earliest()
            .map(|date| date.timestamp_millis())
            .unwrap_or(0)
    };
    Range::new(midnight(date), midnight(date.succ_opt().unwrap_or(date)))
}
pub fn time_label(ts: i64) -> String {
    Local
        .timestamp_millis_opt(ts)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_default()
}
pub fn duration(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    let minutes = seconds / 60;
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}
pub fn empty_tracks() -> Vec<Track> {
    ["Tags", "Computer", "Applications", "Documents"]
        .into_iter()
        .map(|name| Track {
            name,
            segments: Vec::new(),
        })
        .collect()
}
pub fn tracks(events: &[Event], tags: &[Value], ranges: &[Value], day: Range) -> Vec<Track> {
    let mut result = empty_tracks();
    for value in ranges {
        let Some(tag_id) = value["tag_id"].as_i64() else {
            continue;
        };
        let range = Range::new(
            value["start_ts"].as_i64().unwrap_or(0),
            value["end_ts"].as_i64().unwrap_or(0),
        );
        let Some(range) = clip(range, day) else {
            continue;
        };
        let tag = tags.iter().find(|tag| tag["id"].as_i64() == Some(tag_id));
        let name = tag
            .and_then(|t| t["name"].as_str())
            .unwrap_or("Unknown tag");
        let mut segment = Segment::new(range, &tag_id.to_string(), name);
        segment.detail = value["note"].as_str().unwrap_or_default().into();
        if let Some(hex) = tag
            .and_then(|tag| tag["color"].as_str())
            .and_then(|hex| hex.strip_prefix('#'))
            .filter(|hex| hex.len() == 6)
        {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                segment.color = [
                    ((rgb >> 16) & 255) as f32 / 255.0,
                    ((rgb >> 8) & 255) as f32 / 255.0,
                    (rgb & 255) as f32 / 255.0,
                ];
            }
        }
        result[0].segments.push(segment);
    }
    for event in events {
        let Some(range) = clip(Range::new(event.start_ts, event.end_ts), day) else {
            continue;
        };
        match event.bucket.as_str() {
            "afk" => {
                let state = event.data["status"].as_str().unwrap_or("unknown");
                let mut segment = Segment::new(
                    range,
                    state,
                    match state {
                        "active" => "Active",
                        "away" => "Away",
                        _ => "Unobserved",
                    },
                );
                segment.color = match state {
                    "active" => [0.37, 0.65, 0.58],
                    "away" => [0.83, 0.69, 0.40],
                    _ => [0.49, 0.53, 0.58],
                };
                result[1].segments.push(segment);
            }
            "window" => {
                let app = event.data["app"].as_str().unwrap_or("Unknown application");
                let app_name = event.data["app_name"]
                    .as_str()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| process_label(app));
                let title = event.data["title"].as_str().unwrap_or("Untitled window");
                let identity = application_identity(app, app_name);
                let mut segment = Segment::new(range, &identity, app_name);
                segment.detail = title.into();
                segment.app = Some(app.into());
                segment.app_name = Some(app_name.into());
                result[2].segments.push(segment.clone());
                segment.key = format!("{app}\0{title}");
                segment.label = title.into();
                segment.detail = app.into();
                result[3].segments.push(segment);
            }
            "web" => {
                let url = event.data["url"].as_str().unwrap_or_default();
                let title = event.data["title"].as_str().unwrap_or(url);
                let mut segment = Segment::new(range, url, title);
                segment.detail = url.into();
                result[3].segments.push(segment);
            }
            _ => {}
        }
    }
    let observed: Vec<_> = result[1]
        .segments
        .iter()
        .filter(|s| s.key == "active" || s.key == "away")
        .map(|s| s.range)
        .collect();
    let tagged: Vec<_> = result[0].segments.iter().map(|s| s.range).collect();
    for range in uncovered(&observed, &tagged, day) {
        let mut segment = Segment::new(range, "untagged", "Untagged");
        segment.color = [0.64, 0.68, 0.72];
        result[0].segments.push(segment);
    }
    for track in &mut result {
        track.segments.sort_by_key(|s| (s.range.start, s.range.end));
    }
    let known = result[1].segments.clone();
    for pair in known.windows(2) {
        if pair[0].range.end < pair[1].range.start {
            let mut segment = Segment::new(
                Range::new(pair[0].range.end, pair[1].range.start),
                "unknown",
                "Unobserved",
            );
            segment.color = [0.49, 0.53, 0.58];
            result[1].segments.push(segment);
        }
    }
    result[1].segments.sort_by_key(|s| s.range.start);
    result
}
#[cfg(test)]
pub fn fixture(day: Range) -> Vec<Track> {
    fixture_with_apps(
        day,
        &[
            ("Editor".into(), "Editor".into()),
            ("Browser".into(), "Browser".into()),
            ("Terminal".into(), "Terminal".into()),
            ("Mail".into(), "Mail".into()),
            ("Design".into(), "Design".into()),
        ],
    )
}
pub fn fixture_with_apps(day: Range, apps: &[(String, String)]) -> Vec<Track> {
    let mut events = Vec::new();
    let at = |hour: i64| day.start + hour * 3_600_000;
    let fallback = [
        ("Editor", "Editor"),
        ("Browser", "Browser"),
        ("Terminal", "Terminal"),
        ("Mail", "Mail"),
        ("Design", "Design"),
    ];
    let apps: Vec<(&str, &str)> = if apps.is_empty() {
        fallback.to_vec()
    } else {
        apps.iter()
            .map(|(process, name)| (process.as_str(), name.as_str()))
            .collect()
    };
    let documents = [
        "Project plan",
        "Draft notes",
        "Research",
        "Design system",
        "Weekly review",
        "Prototype",
        "Build output",
        "Inbox",
    ];
    let morning = 2222;
    for i in 0..5000 {
        let (index, count, base, span) = if i < morning {
            (i, morning, at(8), 4 * 3_600_000)
        } else {
            (i - morning, 5000 - morning, at(13), 5 * 3_600_000)
        };
        events.push(Event {
            id: i + 1,
            bucket: "window".into(),
            start_ts: base + index * span / count,
            end_ts: base + (index + 1) * span / count,
            data: serde_json::json!({"app":apps[i as usize % apps.len()].0,"app_name":apps[i as usize % apps.len()].1,"title":format!("{} — {}", apps[i as usize % apps.len()].1, documents[i as usize % 8])}),
        });
    }
    for (start, end, status) in [(8, 12, "active"), (12, 13, "away"), (13, 18, "active")] {
        events.push(Event {
            id: events.len() as i64 + 1,
            bucket: "afk".into(),
            start_ts: at(start),
            end_ts: at(end),
            data: serde_json::json!({"status":status}),
        });
    }
    let tags = [
        serde_json::json!({"id":1,"name":"Project Atlas","color":"#778dc3"}),
        serde_json::json!({"id":2,"name":"Studio Paper","color":"#5baf9d"}),
    ];
    let ranges = [
        serde_json::json!({"tag_id":1,"start_ts":at(8),"end_ts":at(10)}),
        serde_json::json!({"tag_id":2,"start_ts":at(13),"end_ts":at(16)}),
    ];
    tracks(&events, &tags, &ranges, day)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_intervals_and_selection_shares_are_exact() {
        let ranges = [Range::new(0, 15), Range::new(5, 20), Range::new(20, 25)];
        assert_eq!(union_duration(&ranges, Range::new(10, 30)), 15);
        assert_eq!(clip(Range::new(0, 10), Range::new(10, 20)), None);
        let segments = vec![
            Segment::new(Range::new(0, 15), "editor", "Editor"),
            Segment::new(Range::new(5, 20), "editor", "Editor"),
        ];
        let rows = summarize(&segments, Range::new(10, 30));
        assert_eq!(rows[0].duration, 10);
        assert_eq!(rows[0].share, 50.0);
    }
    #[test]
    fn drag_and_zoom_are_clamped_and_reversible() {
        assert_eq!(
            pixel_range(Range::new(100, 200), 100.0, 90.0, 10.0),
            Range::new(110, 190)
        );
        assert_eq!(
            pixel_range(Range::new(100, 200), 100.0, -20.0, 120.0),
            Range::new(100, 200)
        );
        assert_eq!(
            zoom(
                Range::new(0, 86_400_000),
                Range::new(0, 86_400_000),
                0.5,
                None
            ),
            Range::new(21_600_000, 64_800_000)
        );
    }
    #[test]
    fn zoom_keeps_the_anchor_at_the_same_screen_position() {
        let day = Range::new(0, 86_400_000);
        let view = Range::new(10_000_000, 20_000_000);
        let anchor = 12_000_000;
        let zoomed = zoom(day, view, 0.9, Some(anchor));
        assert_eq!(zoomed, Range::new(10_200_000, 19_200_000));
        assert_eq!(zoom(day, zoomed, 1.0 / 0.9, Some(anchor)), view);
    }
    #[test]
    fn fixture_contains_5000_windows_four_tracks_and_correct_totals() {
        let day = Range::new(0, 86_400_000);
        let data = fixture(day);
        assert_eq!(data[2].segments.len(), 5000);
        assert_eq!(
            union_duration(
                &data[1]
                    .segments
                    .iter()
                    .filter(|s| s.key == "active")
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                day
            ),
            9 * 3_600_000
        );
        assert_eq!(data.len(), 4);
        assert_eq!(color("Editor"), color("Editor"));
        assert_ne!(color("Editor"), color("Browser"));
    }

    #[test]
    fn fixture_preserves_supplied_process_paths_for_application_identity() {
        let day = Range::new(0, 86_400_000);
        let apps = vec![
            ("/usr/bin/steam".to_string(), "Balatro".to_string()),
            ("/usr/bin/steam".to_string(), "Aseprite".to_string()),
        ];
        let data = fixture_with_apps(day, &apps);
        let process_paths: std::collections::HashSet<_> = data[2]
            .segments
            .iter()
            .filter_map(|segment| segment.app.as_deref())
            .collect();
        assert_eq!(process_paths, ["/usr/bin/steam"].into_iter().collect());
        let names: std::collections::HashSet<_> = data[2]
            .segments
            .iter()
            .filter_map(|segment| segment.app_name.as_deref())
            .collect();
        assert_eq!(names, ["Balatro", "Aseprite"].into_iter().collect());
    }
    #[test]
    fn process_identity_survives_restart_without_merging_other_executables() {
        let segments = vec![
            Segment::new(Range::new(0, 10), "/one/editor", "editor"),
            Segment::new(Range::new(10, 20), "/one/editor", "editor"),
            Segment::new(Range::new(20, 30), "/two/editor", "editor"),
        ];
        let rows = summarize(&segments, Range::new(0, 30));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].duration, 20);
        assert_eq!(continuous_span(&segments[1], &segments), Range::new(0, 20));
    }

    #[test]
    fn application_summary_separates_apps_sharing_a_launcher_process() {
        let day = Range::new(0, 100);
        let events = [
            Event {
                id: 1,
                bucket: "window".into(),
                start_ts: 0,
                end_ts: 10,
                data: serde_json::json!({"app":"/usr/bin/steam","app_name":"Balatro","title":"Balatro"}),
            },
            Event {
                id: 2,
                bucket: "window".into(),
                start_ts: 10,
                end_ts: 20,
                data: serde_json::json!({"app":"/usr/bin/steam","app_name":"Aseprite","title":"Aseprite"}),
            },
        ];
        let tracks = tracks(&events, &[], &[], day);
        let summary = summarize(&tracks[2].segments, day);
        let labels: std::collections::HashSet<_> =
            summary.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Aseprite", "Balatro"].into_iter().collect());
        assert_eq!(summary.len(), 2);
    }
}
