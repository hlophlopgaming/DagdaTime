use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupBy {
    Tag,
    Application,
    Document,
}
impl GroupBy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "tag" => Some(Self::Tag),
            "application" => Some(Self::Application),
            "document" => Some(Self::Document),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ReportInterval {
    pub start_ts: i64,
    pub end_ts: i64,
    pub tag_id: Option<i64>,
    pub tag: String,
    pub application: String,
    pub document: String,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ReportRow {
    pub tag_id: Option<i64>,
    pub label: String,
    pub duration_ms: i64,
    pub share: f64,
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Report {
    pub from: i64,
    pub to: i64,
    pub active_ms: i64,
    pub intervals: Vec<ReportInterval>,
}

pub(crate) fn observed_active(computer: Option<&Event>, afk: Option<&Event>, legacy: bool) -> bool {
    if let Some(c) = computer {
        c.data["machine"] == "awake"
            && c.data["activity"] == "active"
            && c.data["locked"] != true
            && matches!(
                c.data["recording"].as_str(),
                Some("recording" | "unobserved")
            )
    } else {
        legacy && afk.is_some_and(|a| a.data["status"] == "active")
    }
}
impl Report {
    /// Sweep half-open boundaries, assigning each active millisecond exactly once.
    pub fn calculate(
        events: &[Event],
        tags: &[Tag],
        ranges: &[TagRange],
        from: i64,
        to: i64,
    ) -> Result<Self, StorageError> {
        if from < 0 || to <= from {
            return Err(invalid("expected 0 <= from < to"));
        }
        let mut report = Self {
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
                (tag.id, (root.id, root.name.clone()))
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
                .map(|(id, name)| (Some(*id), name.clone()))
                .unwrap_or((None, "Untagged".into()));
            let window = current("window");
            let web = current("web");
            let application = window
                .and_then(|w| {
                    w.data["app_name"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .or_else(|| w.data["app"].as_str())
                })
                .unwrap_or("Unknown application")
                .to_owned();
            let document = web
                .and_then(|e| e.data["url"].as_str())
                .or_else(|| window.and_then(|e| e.data["title"].as_str()))
                .unwrap_or("Unknown document")
                .to_owned();
            report.active_ms += end - start;
            if let Some(last) = report.intervals.last_mut().filter(|last| {
                last.end_ts == start
                    && last.tag_id == tag_id
                    && last.application == application
                    && last.document == document
            }) {
                last.end_ts = end;
            } else {
                report.intervals.push(ReportInterval {
                    start_ts: start,
                    end_ts: end,
                    tag_id,
                    tag,
                    application,
                    document,
                });
            }
        }
        Ok(report)
    }
    pub fn rows(&self, group: GroupBy, filter: Option<i64>) -> Vec<ReportRow> {
        let mut totals: BTreeMap<(Option<i64>, String), i64> = BTreeMap::new();
        for i in self
            .intervals
            .iter()
            .filter(|i| filter.is_none_or(|id| i.tag_id == Some(id)))
        {
            let key = match group {
                GroupBy::Tag => (i.tag_id, i.tag.clone()),
                GroupBy::Application => (None, i.application.clone()),
                GroupBy::Document => (None, i.document.clone()),
            };
            *totals.entry(key).or_default() += i.end_ts - i.start_ts;
        }
        let total = totals.values().sum::<i64>();
        let mut rows: Vec<_> = totals
            .into_iter()
            .map(|((tag_id, label), duration_ms)| ReportRow {
                tag_id,
                label,
                duration_ms,
                share: duration_ms as f64 / total.max(1) as f64,
            })
            .collect();
        rows.sort_by(|a, b| {
            b.duration_ms
                .cmp(&a.duration_ms)
                .then(a.label.cmp(&b.label))
        });
        rows
    }
    pub fn csv(&self, group: GroupBy, filter: Option<i64>) -> String {
        let mut csv = "from,to,tag_id,label,duration_ms,share\r\n".to_owned();
        for r in self.rows(group, filter) {
            csv.push_str(&format!(
                "{},{},{},\"{}\",{},{}\r\n",
                self.from,
                self.to,
                r.tag_id.map(|id| id.to_string()).unwrap_or_default(),
                r.label.replace('"', "\"\""),
                r.duration_ms,
                r.share
            ));
        }
        csv
    }
}
fn invalid(message: &str) -> StorageError {
    StorageError::InvalidRecord(message.into())
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub format: String,
    pub events: Vec<Event>,
    pub tags: Vec<Tag>,
    pub tag_ranges: Vec<TagRange>,
}
impl Storage {
    pub fn report(&self, from: i64, to: i64) -> Result<Report, StorageError> {
        let mut events = self.events(None, from, to)?;
        // Old AFK buckets end permanently when computer evidence starts, including across queries.
        if let Some(first) = self.first_event_start("computer")? {
            for e in &mut events {
                if e.bucket == "afk" {
                    e.end_ts = e.end_ts.min(first);
                }
            }
        }
        Report::calculate(
            &events,
            &self.tags()?,
            &self.tag_ranges(from, to)?,
            from,
            to,
        )
    }
    pub fn export_archive(&self, from: i64, to: i64) -> Result<Archive, StorageError> {
        let mut events = self.events(None, from, to)?;
        if let Some(first) = self.first_event_start("computer")? {
            for e in &mut events {
                if e.bucket == "afk" {
                    e.end_ts = e.end_ts.min(first);
                }
            }
            events.retain(|e| e.bucket != "afk" || e.end_ts > e.start_ts.max(from));
        }
        for e in &mut events {
            e.start_ts = e.start_ts.max(from);
            e.end_ts = e.end_ts.min(to);
        }
        let mut ranges = self.tag_ranges(from, to)?;
        for r in &mut ranges {
            r.start_ts = r.start_ts.max(from);
            r.end_ts = r.end_ts.min(to);
        }
        Ok(Archive {
            format: "draugr-time-v1".into(),
            events,
            tags: self.tags()?,
            tag_ranges: ranges,
        })
    }
    /// Add history atomically. Conflicting intervals are rejected, never overwritten.
    pub fn import_json(&mut self, bytes: &[u8]) -> Result<usize, StorageError> {
        let value: Value = serde_json::from_slice(bytes)?;
        let archive = if value.get("format").is_some() {
            serde_json::from_value::<Archive>(value)?
        } else {
            activitywatch(value)?
        };
        if archive.format != "draugr-time-v1" {
            return Err(invalid("unsupported archive format"));
        }
        // Validate hierarchy and all tag fields through the existing CRUD boundary in isolated storage.
        let mut staging = Storage::open_in_memory()?;
        let mut pending = archive.tags.clone();
        let mut map = HashMap::new();
        while !pending.is_empty() {
            let previous = pending.len();
            let mut remaining = Vec::new();
            for t in pending {
                if t.id <= 0 || map.contains_key(&t.id) {
                    return Err(invalid("invalid or duplicate tag id"));
                }
                if t.parent_id.is_some_and(|id| !map.contains_key(&id)) {
                    remaining.push(t);
                    continue;
                }
                let v=staging.put(Collection::Tags,None,serde_json::json!({"name":t.name,"color":t.color,"parent_id":t.parent_id.map(|id|map[&id]),"archived":t.archived}))?;
                map.insert(t.id, v["id"].as_i64().unwrap());
            }
            if remaining.len() == previous {
                return Err(invalid("missing or cyclic tag parent"));
            }
            pending = remaining;
        }
        let mut imported_ranges = archive.tag_ranges;
        imported_ranges.sort_by_key(|r| r.id);
        if imported_ranges.iter().any(|r| r.id <= 0)
            || imported_ranges.windows(2).any(|p| p[0].id == p[1].id)
        {
            return Err(invalid("invalid or duplicate tag-range id"));
        }
        for r in &imported_ranges {
            staging.put(Collection::TagRanges,None,serde_json::json!({"tag_id":map.get(&r.tag_id).ok_or_else(||invalid("missing tag"))?,"start_ts":r.start_ts,"end_ts":r.end_ts,"note":r.note}))?;
        }
        let mut events = archive.events;
        events.sort_by(|a, b| {
            a.bucket
                .cmp(&b.bucket)
                .then(a.start_ts.cmp(&b.start_ts))
                .then(a.end_ts.cmp(&b.end_ts))
        });
        for (i, e) in events.iter().enumerate() {
            if e.bucket.trim().is_empty()
                || e.start_ts < 0
                || e.end_ts < e.start_ts
                || !e.data.is_object()
            {
                return Err(invalid("invalid imported event"));
            }
            if i > 0 && events[i - 1].bucket == e.bucket && events[i - 1].end_ts > e.start_ts {
                return Err(invalid("overlapping events in import bucket"));
            }
        }
        self.flush()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for e in &events {
            let conflict:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE bucket=?1 AND ((start_ts<?3 AND end_ts>?2) OR (start_ts=?2 AND end_ts=?3)))",params![e.bucket,e.start_ts,e.end_ts],|r|r.get(0))?;
            if conflict {
                return Err(invalid(
                    "import overlaps existing history; use a separate data folder",
                ));
            }
            tx.execute(
                "INSERT INTO events(bucket,start_ts,end_ts,data) VALUES(?1,?2,?3,?4)",
                params![
                    e.bucket,
                    e.start_ts,
                    e.end_ts,
                    serde_json::to_string(&e.data)?
                ],
            )?;
        }
        let mut target = HashMap::new();
        for t in staging.tags()? {
            tx.execute(
                "INSERT INTO tags(name,color,parent_id,archived) VALUES(?1,?2,?3,?4)",
                params![
                    t.name,
                    t.color,
                    t.parent_id.map(|id| target[&id]),
                    t.archived
                ],
            )?;
            target.insert(t.id, tx.last_insert_rowid());
        }
        let mut staged_ranges = staging.tag_ranges(0, i64::MAX)?;
        staged_ranges.sort_by_key(|r| r.id);
        for r in staged_ranges {
            tx.execute(
                "INSERT INTO tag_ranges(tag_id,start_ts,end_ts,note) VALUES(?1,?2,?3,?4)",
                params![target[&r.tag_id], r.start_ts, r.end_ts, r.note],
            )?;
        }
        tx.commit()?;
        self.tails.clear();
        self.auto_tail = None;
        self.metadata_version = self.metadata_version.wrapping_add(1);
        Ok(events.len())
    }
}
fn activitywatch(value: Value) -> Result<Archive, StorageError> {
    let buckets = value
        .get("buckets")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("expected ActivityWatch buckets"))?;
    let mut events = Vec::new();
    for bucket in buckets.values() {
        let name = match bucket["type"].as_str() {
            Some("currentwindow") => "window",
            Some("afkstatus") => "afk",
            Some("web.tab.current") => "web",
            _ => return Err(invalid("unsupported ActivityWatch bucket type")),
        };
        for e in bucket["events"]
            .as_array()
            .ok_or_else(|| invalid("expected bucket events"))?
        {
            let start = chrono::DateTime::parse_from_rfc3339(
                e["timestamp"]
                    .as_str()
                    .ok_or_else(|| invalid("missing timestamp"))?,
            )
            .map_err(|_| invalid("invalid RFC3339 timestamp"))?
            .timestamp_millis();
            let duration = e["duration"]
                .as_f64()
                .filter(|d| d.is_finite() && *d >= 0.0 && *d < (i64::MAX / 1000) as f64)
                .ok_or_else(|| invalid("invalid duration"))?;
            let end = start
                .checked_add((duration * 1000.0).round() as i64)
                .ok_or_else(|| invalid("timestamp overflow"))?;
            let mut data = e["data"].clone();
            if name == "afk" {
                match data["status"].as_str() {
                    Some("not-afk") => data["status"] = Value::from("active"),
                    Some("afk") => data["status"] = Value::from("away"),
                    _ => return Err(invalid("invalid AFK status")),
                };
            }
            events.push(Event {
                id: events.len() as i64 + 1,
                bucket: name.into(),
                start_ts: start,
                end_ts: end,
                data,
            });
        }
    }
    Ok(Archive {
        format: "draugr-time-v1".into(),
        events,
        tags: Vec::new(),
        tag_ranges: Vec::new(),
    })
}
