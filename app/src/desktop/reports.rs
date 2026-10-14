use super::*;
use chrono::Datelike;
use draugr_time_core::{Event, GroupBy, Report, Tag, TagRange};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::widget::column;
use iced::{mouse, Point, Rectangle, Renderer, Size};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub type Highlight = Option<(Option<i64>, Option<String>)>;
#[derive(Clone, Debug)]
pub enum Edit {
    Refresh,
    Loaded(u64, Result<Data, String>),
    From(String),
    To(String),
    Preset(u8),
    Group(usize),
    Filter(Option<i64>),
    Highlight(Highlight),
    Expand,
    OpenDay(NaiveDate),
    Export(bool),
    Import,
    File(Option<std::path::PathBuf>),
    ImportDone(Result<usize, String>),
    ExportDone(Result<(), String>),
    Copy,
}
#[derive(Clone, Debug)]
pub struct Day {
    pub date: NaiveDate,
    pub range: Range,
    pub active_ms: i64,
    pub tag_totals: BTreeMap<Option<i64>, i64>,
    pub bands: [Vec<Band>; 3],
    cache: Arc<Mutex<WeekCache>>,
}
impl Day {
    fn summarize(date: NaiveDate, range: Range, report: &Report) -> Self {
        let mut tag_totals = BTreeMap::new();
        for interval in day_intervals(report, range) {
            let duration = interval.end_ts.min(range.end) - interval.start_ts.max(range.start);
            *tag_totals.entry(interval.tag_id).or_default() += duration;
        }
        Self {
            date,
            range,
            active_ms: tag_totals.values().sum(),
            tag_totals,
            bands: Default::default(),
            cache: Arc::new(Mutex::new(WeekCache::default())),
        }
    }
}
fn day_intervals(
    report: &Report,
    range: Range,
) -> impl Iterator<Item = &draugr_time_core::ReportInterval> {
    let start = report
        .intervals
        .partition_point(|i| i.end_ts <= range.start);
    report.intervals[start..]
        .iter()
        .take_while(move |i| i.start_ts < range.end)
}
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    start: u16,
    end: u16,
    color: [f32; 3],
}
type Cells = [[Option<[f32; 3]>; 1440]; 3];
fn empty_cells() -> Cells {
    [[None; 1440]; 3]
}
fn fill_cells(cells: &mut [Option<[f32; 3]>; 1440], range: Range, color: [f32; 3], day: Range) {
    let Some(range) = model::clip(range, day) else {
        return;
    };
    let duration = day.duration();
    let start = ((range.start - day.start) as i128 * 1440 / duration as i128) as usize;
    let end = (((range.end - day.start) as i128 * 1440 + duration as i128 - 1) / duration as i128)
        as usize;
    cells[start..end.min(1440)].fill(Some(color));
}
fn fill_states(cells: &mut Cells, tracks: &[Track], range: Range) {
    for s in &tracks[1].segments {
        fill_cells(&mut cells[0], s.range, s.color, range);
    }
    for s in tracks[2]
        .segments
        .iter()
        .filter(|s| s.key.as_ref() == "unobserved")
    {
        fill_cells(
            &mut cells[1],
            s.range,
            [195.0 / 255.0, 95.0 / 255.0, 95.0 / 255.0],
            range,
        );
    }
}
fn fill_tags(cells: &mut Cells, report: &Report, range: Range, tags: &[Tag]) {
    for i in day_intervals(report, range) {
        let color = if i.tag_id.is_none() {
            Color::from_rgb8(145, 155, 170)
        } else {
            tag_color(tags, i.tag_id)
        };
        fill_cells(
            &mut cells[2],
            Range::new(i.start_ts, i.end_ts),
            [color.r, color.g, color.b],
            range,
        );
    }
}
fn compress_cells(cells: Cells) -> [Vec<Band>; 3] {
    cells.map(|row| {
        let mut bands: Vec<Band> = Vec::new();
        for (index, color) in row.into_iter().enumerate() {
            let Some(color) = color else {
                continue;
            };
            if let Some(last) = bands
                .last_mut()
                .filter(|last| last.end as usize == index && last.color == color)
            {
                last.end += 1;
            } else {
                bands.push(Band {
                    start: index as u16,
                    end: index as u16 + 1,
                    color,
                });
            }
        }
        bands
    })
}
fn bands_from_tracks(
    tracks: &[Track],
    report: &Report,
    range: Range,
    tags: &[Tag],
) -> [Vec<Band>; 3] {
    let mut cells = empty_cells();
    fill_states(&mut cells, tracks, range);
    fill_tags(&mut cells, report, range, tags);
    compress_cells(cells)
}
#[derive(Debug, Default)]
struct WeekCache {
    geometry: canvas::Cache,
    theme: Option<Theme>,
}
struct WeekCanvas<'a> {
    day: &'a Day,
    stats: &'a timeline::PaintStats,
}
impl canvas::Program<Message> for WeekCanvas<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut cache = self.day.cache.lock().unwrap();
        if cache.theme.as_ref() != Some(theme) {
            cache.geometry.clear();
            cache.theme = Some(theme.clone());
        }
        vec![cache.geometry.draw(renderer, bounds.size(), |frame| {
            let started = Instant::now();
            let colors = theme.extended_palette();
            let row_height = (bounds.height - 18.0) / 2.0;
            frame.fill_rectangle(
                Point::new(0.0, 18.0),
                Size::new(bounds.width, row_height * 2.0),
                colors.background.weak.color,
            );
            for (row, bands) in self.day.bands.iter().enumerate() {
                let (y, height) = match row {
                    1 => (18.0, 6.0),
                    2 => (18.0 + row_height, row_height),
                    _ => (18.0, row_height),
                };
                for band in bands {
                    let x = band.start as f32 / 1440.0 * bounds.width;
                    let width = (band.end - band.start) as f32 / 1440.0 * bounds.width;
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(width, height),
                        Color::from_rgb(band.color[0], band.color[1], band.color[2]),
                    );
                }
            }
            for hour in [0, 6, 12, 18] {
                let ts = self.day.range.start + self.day.range.duration() * hour / 24;
                frame.fill_text(Text {
                    content: model::time_label(ts),
                    position: Point::new(hour as f32 / 24.0 * bounds.width, 0.0),
                    size: 11.0.into(),
                    color: colors.background.base.text,
                    ..Default::default()
                });
            }
            self.stats.frames.fetch_add(1, Ordering::Relaxed);
            self.stats
                .max_build_us
                .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        })]
    }
}
#[derive(Clone, Debug)]
pub struct Data {
    pub from: String,
    pub to: String,
    pub report: Report,
    pub days: Vec<Day>,
}
#[derive(Default)]
pub struct Ui {
    pub from: String,
    pub to: String,
    pub group: usize,
    pub filter: Option<i64>,
    pub expanded: bool,
    pub highlight: Highlight,
    pub generation: u64,
    pub data: Option<Data>,
    pub day: Report,
    pub loading: bool,
    pub error: String,
}
#[derive(Clone, Debug)]
pub struct Slice {
    pub tag_id: Option<i64>,
    pub tag: String,
    pub application: String,
    pub duration: i64,
    pub members: Vec<String>,
}
impl Slice {
    fn key(&self) -> Highlight {
        Some((self.tag_id, Some(self.application.clone())))
    }
}
pub fn slices(report: &Report, expanded: bool) -> Vec<Slice> {
    let mut groups: BTreeMap<(Option<i64>, &str), BTreeMap<&str, i64>> = BTreeMap::new();
    for i in &report.intervals {
        *groups
            .entry((i.tag_id, i.tag.as_ref()))
            .or_default()
            .entry(i.application.as_ref())
            .or_default() += i.end_ts - i.start_ts;
    }
    let mut slices = Vec::new();
    for ((tag_id, tag), apps) in groups {
        let mut other = Slice {
            tag_id,
            tag: tag.into(),
            application: "Other".into(),
            duration: 0,
            members: Vec::new(),
        };
        for (application, duration) in apps {
            if !expanded && (duration as f64) < report.active_ms as f64 * 0.02 {
                other.duration += duration;
                other.members.push(application.into());
            } else {
                slices.push(Slice {
                    tag_id,
                    tag: tag.into(),
                    application: application.into(),
                    duration,
                    members: Vec::new(),
                });
            }
        }
        if other.duration > 0 {
            slices.push(other);
        }
    }
    slices
}
/// Convert already classified Day tracks to the shared calculation, preserving Day's activity basis.
pub fn from_tracks(tracks: &[Track], tags: &[Tag], range: Range) -> Result<Report, String> {
    let mut events = Vec::new();
    for s in &tracks[1].segments {
        if s.key.as_ref() == "active" {
            events.push(Event {
                id: events.len() as i64 + 1,
                bucket: "afk".into(),
                start_ts: s.range.start,
                end_ts: s.range.end,
                data: serde_json::json!({"status":"active"}),
            });
        }
    }
    for s in &tracks[3].segments {
        let data = match s.bucket {
            "window" => {
                serde_json::json!({"app":s.app.as_deref(),"app_name":s.app_name.as_deref(),"title":s.label.as_ref()})
            }
            "web" => serde_json::json!({"url":s.detail.as_ref()}),
            _ => continue,
        };
        events.push(Event {
            id: events.len() as i64 + 1,
            bucket: s.bucket.into(),
            start_ts: s.range.start,
            end_ts: s.range.end,
            data,
        });
    }
    let mut tags = tags.to_vec();
    let mut ranges = Vec::new();
    for s in &tracks[0].segments {
        if let Some((id, original)) = s.tag_record {
            let tag_id = s.key.parse::<i64>().map_err(|e| e.to_string())?;
            if !tags.iter().any(|t| t.id == tag_id) {
                tags.push(Tag {
                    id: tag_id,
                    name: s.label.to_string(),
                    color: None,
                    parent_id: None,
                    archived: 0,
                });
            }
            ranges.push(TagRange {
                id,
                tag_id,
                start_ts: original.start,
                end_ts: original.end,
                note: None,
            });
        }
    }
    Report::calculate(&events, &tags, &ranges, range.start, range.end).map_err(|e| e.to_string())
}
fn date_range(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate, Range), String> {
    let from =
        NaiveDate::parse_from_str(from, "%Y-%m-%d").map_err(|_| tr("Use YYYY-MM-DD dates"))?;
    let to = NaiveDate::parse_from_str(to, "%Y-%m-%d").map_err(|_| tr("Use YYYY-MM-DD dates"))?;
    if to < from || (to - from).num_days() > 365 {
        return Err(tr("Choose up to 366 days"));
    }
    let range = Range::new(model::day_bounds(from).start, model::day_bounds(to).end);
    if range.start < 0 || range.duration() == 0 {
        return Err(tr("Choose a valid date range"));
    }
    Ok((from, to, range))
}
fn load(
    shared: draugr_time_daemon::Shared,
    from: String,
    to: String,
    week: bool,
) -> Result<Data, String> {
    let (start, end, range) = date_range(&from, &to)?;
    let db = shared.storage.lock().map_err(|e| e.to_string())?;
    let report = db
        .report(range.start, range.end)
        .map_err(|e| e.to_string())?;
    let tags = if week {
        db.tags().map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let first_computer = if week {
        db.first_event_start("computer")
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let mut days = Vec::new();
    let mut date = start;
    loop {
        let range = model::day_bounds(date);
        let mut day = Day::summarize(date, range, &report);
        if week {
            let mut cells = empty_cells();
            fill_tags(&mut cells, &report, range, &tags);
            for bucket in ["computer", "afk"] {
                let mut cursor = range.start;
                while cursor < range.end {
                    let (mut events, mut next) = db
                        .event_chunk(Some(bucket), cursor, range.end, 8193)
                        .map_err(|e| e.to_string())?;
                    if next <= cursor {
                        next = cursor + 1;
                        events = db
                            .event_chunk(Some(bucket), cursor, next, i64::MAX)
                            .map_err(|e| e.to_string())?
                            .0;
                    }
                    events.retain(|e| e.start_ts < next);
                    for e in &mut events {
                        e.start_ts = e.start_ts.max(cursor);
                        e.end_ts = e.end_ts.min(next);
                        if bucket == "afk" {
                            e.end_ts = e.end_ts.min(first_computer.unwrap_or(range.end));
                        }
                    }
                    let tracks = model::tracks(&events, &[], &[], range);
                    fill_states(&mut cells, &tracks, range);
                    cursor = next;
                }
            }
            day.bands = compress_cells(cells);
        }
        days.push(day);
        if date == end {
            break;
        }
        date = date
            .succ_opt()
            .ok_or_else(|| tr("Choose a valid date range"))?;
    }
    Ok(Data {
        from,
        to,
        report,
        days,
    })
}
impl State {
    pub(super) fn release_reports(&mut self) {
        self.report_ui.data = None;
        self.report_ui.loading = false;
        self.report_ui.generation = self.report_ui.generation.wrapping_add(1);
        memory::SystemReclaimer.reclaim();
    }
    pub fn edit_report(&mut self, event: Edit) -> Task<Message> {
        match event {
            Edit::From(s) => {
                self.report_ui.from = s;
                self.report_ui.data = None;
                self.report_ui.generation += 1;
                self.report_ui.loading = false;
            }
            Edit::To(s) => {
                self.report_ui.to = s;
                self.report_ui.data = None;
                self.report_ui.generation += 1;
                self.report_ui.loading = false;
            }
            Edit::Preset(p) => {
                let from = match p {
                    1 => self
                        .date
                        .checked_sub_days(Days::new(u64::from(
                            self.date.weekday().num_days_from_monday(),
                        )))
                        .unwrap_or(self.date),
                    2 => self.date.with_day(1).unwrap_or(self.date),
                    _ => self.date,
                };
                let to = match p {
                    1 => from.checked_add_days(Days::new(6)).unwrap_or(from),
                    2 => from
                        .checked_add_months(chrono::Months::new(1))
                        .and_then(|d| d.pred_opt())
                        .unwrap_or(from),
                    _ => from,
                };
                self.report_ui.from = from.to_string();
                self.report_ui.to = to.to_string();
                return self.edit_report(Edit::Refresh);
            }
            Edit::Group(g) => self.report_ui.group = g,
            Edit::Filter(id) => self.report_ui.filter = id,
            Edit::Highlight(h) => self.report_ui.highlight = h,
            Edit::Expand => self.report_ui.expanded = !self.report_ui.expanded,
            Edit::OpenDay(date) => {
                self.set_screen("Day");
                self.change_date(date);
            }
            Edit::Refresh => {
                if self.screen == "Overview" {
                    return Task::none();
                }
                if self.screen == "Week" {
                    let start = self
                        .date
                        .checked_sub_days(Days::new(u64::from(
                            self.date.weekday().num_days_from_monday(),
                        )))
                        .unwrap_or(self.date);
                    self.report_ui.from = start.to_string();
                    self.report_ui.to = start
                        .checked_add_days(Days::new(6))
                        .unwrap_or(start)
                        .to_string();
                } else if self.report_ui.from.is_empty() {
                    self.report_ui.from = self.date.to_string();
                    self.report_ui.to = self.date.to_string();
                }
                if let Err(error) = date_range(&self.report_ui.from, &self.report_ui.to) {
                    self.report_ui.error = error;
                    return Task::none();
                }
                self.report_ui.generation += 1;
                let generation = self.report_ui.generation;
                self.report_ui.error.clear();
                self.report_ui.data = None;
                if self.fixture {
                    let (start, end, _) =
                        date_range(&self.report_ui.from, &self.report_ui.to).unwrap();
                    let mut date = start;
                    let mut days = Vec::new();
                    let mut total = Report::default();
                    loop {
                        let range = model::day_bounds(date);
                        let tracks = model::fixture_with_apps(range, &self.preview_apps);
                        let report = from_tracks(&tracks, &[], range).unwrap();
                        total.active_ms += report.active_ms;
                        let mut day = Day::summarize(date, range, &report);
                        if self.screen == "Week" {
                            day.bands = bands_from_tracks(&tracks, &report, range, &[]);
                        }
                        days.push(day);
                        total.intervals.extend(report.intervals);
                        if date == end {
                            break;
                        }
                        date = date.succ_opt().unwrap();
                    }
                    total.from = days[0].range.start;
                    total.to = days.last().unwrap().range.end;
                    self.report_ui.data = Some(Data {
                        from: self.report_ui.from.clone(),
                        to: self.report_ui.to.clone(),
                        report: total,
                        days,
                    });
                    return Task::none();
                }
                let Some(runtime) = &self.runtime else {
                    self.report_ui.error = tr("Start recording to load history");
                    return Task::none();
                };
                let shared = runtime.shared.clone();
                let from = self.report_ui.from.clone();
                let to = self.report_ui.to.clone();
                let week = self.screen == "Week";
                self.report_ui.loading = true;
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || load(shared, from, to, week))
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|v| v)
                    },
                    move |r| Message::Report(Edit::Loaded(generation, r)),
                );
            }
            Edit::Loaded(g, r) => {
                if g != self.report_ui.generation {
                    drop(r);
                    memory::SystemReclaimer.reclaim();
                    return Task::none();
                }
                self.report_ui.loading = false;
                match r {
                    Ok(data) => {
                        self.report_ui.data = Some(data);
                        self.report_ui.error.clear();
                    }
                    Err(e) => self.report_ui.error = e,
                }
            }
            Edit::Copy => {
                let Some(data) = &self.report_ui.data else {
                    return Task::none();
                };
                return iced::clipboard::write(
                    data.report
                        .csv(group(self.report_ui.group), self.report_ui.filter),
                );
            }
            Edit::Export(json) => {
                let Some(data) = &self.report_ui.data else {
                    return Task::none();
                };
                let report = data.report.clone();
                let grouping = group(self.report_ui.group);
                let filter = self.report_ui.filter;
                let shared = self.runtime.as_ref().map(|r| r.shared.clone());
                return Task::perform(
                    async move {
                        let Some(file) = rfd::AsyncFileDialog::new()
                            .set_file_name(if json {
                                "draugr-history.json"
                            } else {
                                "draugr-report.csv"
                            })
                            .save_file()
                            .await
                        else {
                            return Ok(());
                        };
                        tokio::task::spawn_blocking(move || {
                            let bytes = if json {
                                let shared = shared.ok_or_else(|| {
                                    "Start recording to export history".to_owned()
                                })?;
                                let archive = shared
                                    .storage
                                    .lock()
                                    .map_err(|e| e.to_string())?
                                    .export_archive(report.from, report.to)
                                    .map_err(|e| e.to_string())?;
                                serde_json::to_vec_pretty(&archive).map_err(|e| e.to_string())?
                            } else {
                                report.csv(grouping, filter).into_bytes()
                            };
                            // Atomic creation: an existing destination is never truncated by a failed export.
                            let mut output = std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(file.path())
                                .map_err(|e| e.to_string())?;
                            use std::io::Write;
                            output.write_all(&bytes).map_err(|e| e.to_string())
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|v| v)
                    },
                    |r| Message::Report(Edit::ExportDone(r)),
                );
            }
            Edit::ExportDone(r) => {
                self.report_ui.error = r.err().unwrap_or_else(|| tr("Export complete"))
            }
            Edit::Import => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |p| Message::Report(Edit::File(p)),
                )
            }
            Edit::File(Some(path)) => {
                let Some(runtime) = &self.runtime else {
                    self.error = tr("Start recording to import history");
                    return Task::none();
                };
                let shared = runtime.shared.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                            shared
                                .storage
                                .lock()
                                .map_err(|e| e.to_string())?
                                .import_json(&bytes)
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|v| v)
                    },
                    |r| Message::Report(Edit::ImportDone(r)),
                );
            }
            Edit::File(None) => {}
            Edit::ImportDone(r) => match r {
                Ok(count) => {
                    self.error = format!("{}: {count}", tr("Imported events"));
                    self.invalidate_load();
                    self.report_ui.data = None;
                }
                Err(e) => self.error = e,
            },
        }
        Task::none()
    }
    pub fn reports_view(&self) -> Element<'_, Message> {
        if self.screen == "Overview" {
            return self.overview_view();
        }
        let ui = &self.report_ui;
        let mut body = column![text(tr(self.screen)).size(22)].spacing(8);
        if self.screen == "Reports" {
            body = body.push(
                row![
                    button(text(tr("Day"))).on_press(Message::Report(Edit::Preset(0))),
                    button(text(tr("Week"))).on_press(Message::Report(Edit::Preset(1))),
                    button(text(tr("Month"))).on_press(Message::Report(Edit::Preset(2)))
                ]
                .spacing(6),
            );
            body = body.push(
                row![
                    text_input("YYYY-MM-DD", &ui.from).on_input(|s| Message::Report(Edit::From(s))),
                    text_input("YYYY-MM-DD", &ui.to).on_input(|s| Message::Report(Edit::To(s))),
                    button(text(tr("Load report"))).on_press(Message::Report(Edit::Refresh))
                ]
                .spacing(6),
            );
        } else {
            body = body.push(button(text(tr("Refresh"))).on_press(Message::Report(Edit::Refresh)));
        }
        if ui.loading {
            body = body.push(text(tr("Loading…")));
        }
        if !ui.error.is_empty() {
            body = body.push(text(localization::error(&ui.error)));
        }
        let Some(data) = &ui.data else {
            return body.push(text(tr("Choose dates and load a report"))).into();
        };
        body = body.push(text(format!(
            "{} – {} · {}: {}",
            data.from,
            data.to,
            tr("Active"),
            model::duration(data.report.active_ms)
        )));
        if self.screen == "Week" {
            for day in &data.days {
                body = body.push(
                    button(text(format!(
                        "{} · {}",
                        day.date,
                        model::duration(day.active_ms)
                    )))
                    .on_press(Message::Report(Edit::OpenDay(day.date))),
                );
                body = body.push(
                    Canvas::new(WeekCanvas {
                        day,
                        stats: &self.stats,
                    })
                    .width(Length::Fill)
                    .height(52),
                );
            }
            for r in data.report.rows(GroupBy::Tag, None) {
                body = body.push(text(format!(
                    "{} · {}",
                    translate_label(&r.label),
                    model::duration(r.duration_ms)
                )));
            }
        } else {
            body = body.push(
                row![
                    button(text(tr("Tags"))).on_press(Message::Report(Edit::Group(0))),
                    button(text(tr("Applications"))).on_press(Message::Report(Edit::Group(1))),
                    button(text(tr("Documents"))).on_press(Message::Report(Edit::Group(2)))
                ]
                .spacing(6),
            );
            let mut filters =
                column![button(text(tr("All tags"))).on_press(Message::Report(Edit::Filter(None)))]
                    .spacing(3);
            for r in data.report.rows(GroupBy::Tag, None) {
                if let Some(id) = r.tag_id {
                    filters = filters.push(
                        button(text(r.label)).on_press(Message::Report(Edit::Filter(Some(id)))),
                    );
                }
            }
            body = body.push(scrollable(filters).height(85));
            for r in data.report.rows(group(ui.group), ui.filter) {
                body = body.push(
                    row![
                        text(translate_label(&r.label)).width(Length::Fill),
                        text(model::duration(r.duration_ms)).width(85),
                        text(format!("{:.1}%", r.share * 100.0)).width(65)
                    ]
                    .spacing(6),
                );
                let share = (r.share * 1000.0).round().clamp(1.0, 999.0) as u16;
                body = body.push(row![
                    container(space::horizontal())
                        .height(5)
                        .width(Length::FillPortion(share))
                        .style(|_: &Theme| container::Style {
                            background: Some(Color::from_rgb8(100, 140, 195).into()),
                            ..Default::default()
                        }),
                    space::horizontal().width(Length::FillPortion(1000 - share))
                ]);
            }
            body = body.push(
                row![
                    button(text(tr("Export CSV"))).on_press(Message::Report(Edit::Export(false))),
                    button(text(tr("Export JSON"))).on_press(Message::Report(Edit::Export(true))),
                    button(text(tr("Copy as text"))).on_press(Message::Report(Edit::Copy))
                ]
                .spacing(6),
            );
            body = body.push(text(tr("Timesheet · tags × local days")).size(18));
            let mut table = row![text(tr("Tags")).width(150)].spacing(8);
            for day in &data.days {
                table = table.push(text(day.date.format("%m-%d").to_string()).width(80));
            }
            table = table.push(text(tr("Total")).width(80));
            let mut sheet = column![table].spacing(5);
            for r in data.report.rows(GroupBy::Tag, ui.filter) {
                let mut line = row![text(translate_label(&r.label)).width(150)].spacing(8);
                for day in &data.days {
                    let ms = day.tag_totals.get(&r.tag_id).copied().unwrap_or(0);
                    line = line.push(text(model::duration(ms)).width(80));
                }
                sheet = sheet.push(line.push(text(model::duration(r.duration_ms)).width(80)));
            }
            let mut totals = row![text(tr("Total")).width(150)].spacing(8);
            for day in &data.days {
                let total = day
                    .tag_totals
                    .iter()
                    .filter(|(id, _)| ui.filter.is_none_or(|filter| **id == Some(filter)))
                    .map(|(_, ms)| *ms)
                    .sum();
                totals = totals.push(text(model::duration(total)).width(80));
            }
            sheet = sheet.push(
                totals.push(
                    text(model::duration(
                        data.report
                            .rows(GroupBy::Tag, ui.filter)
                            .iter()
                            .map(|r| r.duration_ms)
                            .sum(),
                    ))
                    .width(80),
                ),
            );
            body = body.push(
                scrollable(sheet).direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                )),
            );
        }
        scrollable(body).into()
    }
    fn overview_view(&self) -> Element<'_, Message> {
        let report = &self.report_ui.day;
        let chart_slices = slices(report, self.report_ui.expanded);
        let mut body=column![text(tr("Day overview")).size(22),text(tr("Overlapping tags use the newest range; child tags count under their top-level project."))].spacing(8);
        if report.active_ms == 0 {
            body=body.push(text(tr("No observed active time for this day. Inactive time and recording gaps remain on the timeline.")));
        } else {
            body = body.push(text(tr("↑ / ↓ highlight · E expand · Esc clear")));
            body = body.push(
                widget::responsive(move |size| {
                    let chart: Element<'_, Message> = Canvas::new(Donut {
                        report,
                        slices: chart_slices.clone(),
                        tags: &self.tags,
                        stats: &self.stats,
                        highlight: self.report_ui.highlight.clone(),
                    })
                    .width(Length::Fill)
                    .height(360)
                    .into();
                    let mut legend = column![].spacing(4);
                    let mut previous = None;
                    for slice in &chart_slices {
                        let key = (slice.tag_id, slice.tag.clone());
                        if previous.as_ref() != Some(&key) {
                            let total = report
                                .intervals
                                .iter()
                                .filter(|i| i.tag_id == slice.tag_id)
                                .map(|i| i.end_ts - i.start_ts)
                                .sum::<i64>();
                            legend = legend.push(
                                button(text(format!(
                                    "{} · {} · {:.1}%",
                                    translate_label(&slice.tag),
                                    model::duration(total),
                                    total as f64 / report.active_ms as f64 * 100.0
                                )))
                                .on_press(Message::Report(Edit::Highlight(Some((
                                    slice.tag_id,
                                    None,
                                )))))
                                .style(
                                    if self.report_ui.highlight == Some((slice.tag_id, None)) {
                                        button::primary
                                    } else {
                                        button::text
                                    },
                                ),
                            );
                            previous = Some(key);
                        }
                        let label = format!(
                            "{} · {} · {:.1}%",
                            translate_label(&slice.application),
                            model::duration(slice.duration),
                            slice.duration as f64 / report.active_ms as f64 * 100.0
                        );
                        legend = legend.push(tooltip(
                            button(text(label.clone()).size(13))
                                .style(if self.report_ui.highlight == slice.key() {
                                    button::primary
                                } else {
                                    button::text
                                })
                                .on_press(Message::Report(Edit::Highlight(slice.key()))),
                            text(label),
                            tooltip::Position::FollowCursor,
                        ));
                        if !slice.members.is_empty() {
                            legend = legend.push(text(slice.members.join(", ")).size(11));
                        }
                    }
                    legend = legend
                        .push(
                            button(text(tr(if self.report_ui.expanded {
                                "Collapse small slices"
                            } else {
                                "Expand small slices"
                            })))
                            .on_press(Message::Report(Edit::Expand)),
                        )
                        .push(
                            button(text(tr("Clear highlight")))
                                .on_press(Message::Report(Edit::Highlight(None))),
                        );
                    if size.width >= 750.0 {
                        row![chart, scrollable(legend).width(Length::Fill).height(360)]
                            .spacing(12)
                            .into()
                    } else {
                        column![chart, scrollable(legend).height(220)]
                            .spacing(8)
                            .into()
                    }
                })
                .height(Length::Shrink),
            );
        }
        body = body.push(
            Canvas::new(Compact {
                tracks: &self.all,
                range: self.day,
                report,
                highlight: self.report_ui.highlight.as_ref(),
                tags: &self.tags,
                stats: &self.stats,
                other: slices(report, false)
                    .into_iter()
                    .filter(|s| s.application == "Other")
                    .collect(),
            })
            .width(Length::Fill)
            .height(88),
        );
        body = body
            .push(text(tr(
                "Tagged / untagged · AFK · sleep / off · pause · no recording · Unobserved",
            )))
            .push(button(text(tr("Back to day"))).on_press(Message::Screen("Day")));
        scrollable(body).into()
    }
}
fn group(i: usize) -> GroupBy {
    match i {
        1 => GroupBy::Application,
        2 => GroupBy::Document,
        _ => GroupBy::Tag,
    }
}
fn translate_label(s: &str) -> String {
    if [
        "Untagged",
        "Unknown application",
        "Unknown document",
        "Other",
    ]
    .contains(&s)
    {
        tr(s)
    } else {
        s.into()
    }
}
fn tag_color(tags: &[Tag], id: Option<i64>) -> Color {
    let rgb = id
        .and_then(|id| tags.iter().find(|t| t.id == id))
        .and_then(|t| t.color.as_deref())
        .and_then(|s| s.strip_prefix('#'))
        .and_then(|s| u32::from_str_radix(s, 16).ok());
    if let Some(rgb) = rgb {
        Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    } else {
        let rgb = model::color(
            &id.map(|v| v.to_string())
                .unwrap_or_else(|| "untagged".into()),
        );
        Color::from_rgb(rgb[0], rgb[1], rgb[2])
    }
}
fn selected(i: &draugr_time_core::ReportInterval, h: &Highlight, other: &[Slice]) -> bool {
    h.as_ref().is_some_and(|(tag, app)| {
        i.tag_id == *tag
            && app.as_ref().is_none_or(|app| {
                i.application.as_ref() == app
                    || (app == "Other"
                        && other.iter().any(|s| {
                            s.tag_id == *tag
                                && s.members.iter().any(|m| m == i.application.as_ref())
                        }))
            })
    })
}
struct Compact<'a> {
    tracks: &'a [Track],
    range: Range,
    report: &'a Report,
    highlight: Option<&'a (Option<i64>, Option<String>)>,
    other: Vec<Slice>,
    tags: &'a [Tag],
    stats: &'a timeline::PaintStats,
}
impl canvas::Program<Message> for Compact<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let started = Instant::now();
        let mut frame = Frame::new(renderer, bounds.size());
        let x = |ts| {
            ((ts - self.range.start) as f64 / self.range.duration().max(1) as f64
                * bounds.width as f64) as f32
        };
        let colors = theme.extended_palette();
        let row_height = (bounds.height - 18.0) / 2.0;
        frame.fill_rectangle(
            Point::new(0.0, 18.0),
            Size::new(bounds.width, row_height * 2.0),
            colors.background.weak.color,
        );
        for s in &self.tracks[1].segments {
            frame.fill_rectangle(
                Point::new(x(s.range.start), 18.0),
                Size::new((x(s.range.end) - x(s.range.start)).max(0.6), row_height),
                Color::from_rgb(s.color[0], s.color[1], s.color[2]),
            );
        }
        for segment in self.tracks[2]
            .segments
            .iter()
            .filter(|s| s.key.as_ref() == "unobserved")
        {
            frame.fill_rectangle(
                Point::new(x(segment.range.start), 18.0),
                Size::new(
                    (x(segment.range.end) - x(segment.range.start)).max(0.6),
                    6.0,
                ),
                Color::from_rgb8(195, 95, 95),
            );
        }
        for i in &self.report.intervals {
            let color = if selected(i, &self.highlight.cloned(), &self.other) {
                colors.primary.strong.color
            } else if i.tag_id.is_none() {
                Color::from_rgb8(145, 155, 170)
            } else {
                tag_color(self.tags, i.tag_id)
            };
            frame.fill_rectangle(
                Point::new(x(i.start_ts), 18.0 + row_height),
                Size::new((x(i.end_ts) - x(i.start_ts)).max(0.6), row_height),
                color,
            );
        }
        for hour in [0, 6, 12, 18] {
            let ts = self.range.start + self.range.duration() * hour / 24;
            frame.fill_text(Text {
                content: model::time_label(ts),
                position: Point::new(x(ts), 0.0),
                size: 11.0.into(),
                color: colors.background.base.text,
                ..Default::default()
            });
        }
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.stats
            .max_build_us
            .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        vec![frame.into_geometry()]
    }
}
struct Donut<'a> {
    report: &'a Report,
    slices: Vec<Slice>,
    tags: &'a [Tag],
    stats: &'a timeline::PaintStats,
    highlight: Highlight,
}
impl Donut<'_> {
    fn hit(&self, p: Point, size: Size) -> Highlight {
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let dx = p.x - center.x;
        let dy = p.y - center.y;
        let radius = dx.hypot(dy);
        let outer = size.width.min(size.height) * 0.44;
        if radius < outer * 0.46
            || radius > outer
            || (radius > outer * 0.72 && radius < outer * 0.75)
        {
            return None;
        }
        let angle = (dy.atan2(dx) + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        let mut position = 0.0;
        for s in &self.slices {
            position +=
                s.duration as f32 / self.report.active_ms.max(1) as f32 * std::f32::consts::TAU;
            if angle < position {
                return Some((
                    s.tag_id,
                    if radius > outer * 0.73 {
                        Some(s.application.clone())
                    } else {
                        None
                    },
                ));
            }
        }
        None
    }
}
impl canvas::Program<Message> for Donut<'_> {
    type State = ();
    fn update(
        &self,
        _: &mut (),
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if matches!(
            event,
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            return cursor
                .position_in(bounds)
                .and_then(|p| self.hit(p, bounds.size()))
                .map(|h| {
                    canvas::Action::publish(Message::Report(Edit::Highlight(Some(h)))).and_capture()
                });
        }
        None
    }
    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor
            .position_in(bounds)
            .and_then(|p| self.hit(p, bounds.size()))
            .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let started = Instant::now();
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.width.min(bounds.height) * 0.44;
        let mut angle = -std::f32::consts::FRAC_PI_2;
        let hover = cursor
            .position_in(bounds)
            .and_then(|p| self.hit(p, bounds.size()));
        let mut tooltip = None;
        for (index, s) in self.slices.iter().enumerate() {
            let end = angle
                + s.duration as f32 / self.report.active_ms.max(1) as f32 * std::f32::consts::TAU;
            let base = tag_color(self.tags, s.tag_id);
            let shade = 0.65 + (index % 5) as f32 * 0.07;
            let app_color = Color {
                r: base.r * shade,
                g: base.g * shade,
                b: base.b * shade,
                ..base
            };
            let first = index == 0 || self.slices[index - 1].tag_id != s.tag_id;
            let project_end = angle
                + self
                    .slices
                    .iter()
                    .skip(index)
                    .take_while(|slice| slice.tag_id == s.tag_id)
                    .map(|slice| slice.duration)
                    .sum::<i64>() as f32
                    / self.report.active_ms.max(1) as f32
                    * std::f32::consts::TAU;
            for (inner, outer, color, sector_end) in [
                (radius * 0.46, radius * 0.72, base, project_end),
                (radius * 0.75, radius, app_color, end),
            ] {
                if inner == radius * 0.46 && !first {
                    continue;
                }
                let steps = ((sector_end - angle) * radius / 3.0).ceil().max(2.0) as usize;
                let path = Path::new(|b| {
                    for j in 0..=steps {
                        let a = angle + (sector_end - angle) * j as f32 / steps as f32;
                        let p = Point::new(center.x + outer * a.cos(), center.y + outer * a.sin());
                        if j == 0 {
                            b.move_to(p);
                        } else {
                            b.line_to(p);
                        }
                    }
                    for j in (0..=steps).rev() {
                        let a = angle + (sector_end - angle) * j as f32 / steps as f32;
                        b.line_to(Point::new(
                            center.x + inner * a.cos(),
                            center.y + inner * a.sin(),
                        ));
                    }
                    b.close();
                });
                frame.fill(&path, color);
                if self.highlight.as_ref().is_some_and(|(tag, app)| {
                    *tag == s.tag_id && app.as_ref().is_none_or(|a| a == &s.application)
                }) || hover.as_ref().is_some_and(|(tag, app)| {
                    *tag == s.tag_id && app.as_ref().is_none_or(|a| a == &s.application)
                }) {
                    frame.stroke(
                        &path,
                        Stroke::default()
                            .with_color(theme.extended_palette().background.base.text)
                            .with_width(2.0),
                    );
                }
            }
            if hover
                .as_ref()
                .is_some_and(|(tag, app)| *tag == s.tag_id && app.as_ref() == Some(&s.application))
            {
                tooltip = Some(format!(
                    "{} · {} · {:.1}%",
                    translate_label(&s.application),
                    model::duration(s.duration),
                    s.duration as f64 / self.report.active_ms as f64 * 100.0
                ));
            }
            angle = end;
        }
        if let Some((tag, None)) = &hover {
            let total = self
                .report
                .intervals
                .iter()
                .filter(|i| i.tag_id == *tag)
                .map(|i| i.end_ts - i.start_ts)
                .sum::<i64>();
            let name = self
                .report
                .intervals
                .iter()
                .find(|i| i.tag_id == *tag)
                .map(|i| translate_label(&i.tag))
                .unwrap_or_default();
            tooltip = Some(format!(
                "{name} · {} · {:.1}%",
                model::duration(total),
                total as f64 / self.report.active_ms.max(1) as f64 * 100.0
            ));
        }
        frame.fill_text(Text {
            content: format!(
                "{}\n{}",
                model::duration(self.report.active_ms),
                tr("Active")
            ),
            position: Point::new(center.x, center.y - 15.0),
            align_x: iced::alignment::Horizontal::Center.into(),
            color: theme.extended_palette().background.base.text,
            size: 20.0.into(),
            ..Default::default()
        });
        if let Some(label) = tooltip {
            frame.fill_text(Text {
                content: label,
                position: Point::new(8.0, bounds.height - 16.0),
                color: theme.extended_palette().background.base.text,
                size: 12.0.into(),
                ..Default::default()
            });
        }
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.stats
            .max_build_us
            .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas::Program;
    #[test]
    fn native_donut_pointer_selects_project_or_application_and_shows_cursor() {
        let report = Report {
            from: 0,
            to: 100,
            active_ms: 100,
            intervals: vec![draugr_time_core::ReportInterval {
                start_ts: 0,
                end_ts: 100,
                tag_id: Some(1),
                tag: "Work".into(),
                application: "Editor".into(),
                document: "draft".into(),
            }],
        };
        let stats = timeline::PaintStats::default();
        let donut = Donut {
            report: &report,
            slices: slices(&report, false),
            tags: &[],
            stats: &stats,
            highlight: None,
        };
        let bounds = Rectangle::with_size(Size::new(300.0, 300.0));
        let outer = mouse::Cursor::Available(Point::new(150.0, 35.0));
        let inner = mouse::Cursor::Available(Point::new(150.0, 70.0));
        for (cursor, app) in [(outer, Some("Editor".to_owned())), (inner, None)] {
            let action = donut
                .update(
                    &mut (),
                    &iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    bounds,
                    cursor,
                )
                .unwrap();
            let (message, _, status) = action.into_inner();
            assert_eq!(status, iced::event::Status::Captured);
            assert!(
                matches!(message,Some(Message::Report(Edit::Highlight(Some((Some(1),value))))) if value==app)
            );
            assert_eq!(
                donut.mouse_interaction(&(), bounds, cursor),
                mouse::Interaction::Pointer
            );
        }
        assert_eq!(donut.hit(Point::new(150.0, 150.0), bounds.size()), None);
        let renderer = Renderer::new(iced::Font::DEFAULT, 16.0.into());
        assert_eq!(
            donut
                .draw(&(), &renderer, &native_theme(true), bounds, outer)
                .len(),
            1
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn week_and_report_totals_equal_day_tracks_and_share_core_rows() {
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let day = model::day_bounds(date);
        {
            let mut db = shared.storage.lock().unwrap();
            db.record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                day.end - 60_000,
                day.end + 120_000,
            )
            .unwrap();
            db.record_interval(
                "window",
                &serde_json::json!({"app":"editor","app_name":"Editor","title":"draft"}),
                day.end - 60_000,
                day.end + 120_000,
            )
            .unwrap();
            db.tag_interval("Client,Task", "", day.end - 60_000, day.end + 60_000, false)
                .unwrap();
        }
        let data = load(
            shared.clone(),
            date.to_string(),
            date.succ_opt().unwrap().to_string(),
            true,
        )
        .unwrap();
        let tags = shared.storage.lock().unwrap().tags().unwrap();
        assert_eq!(
            data.report.active_ms,
            data.days.iter().map(|d| d.active_ms).sum::<i64>()
        );
        for d in &data.days {
            assert_eq!(
                {
                    let db = shared.storage.lock().unwrap();
                    let tracks = model::tracks(
                        &db.events(None, d.range.start, d.range.end).unwrap(),
                        &tags,
                        &db.tag_ranges(d.range.start, d.range.end).unwrap(),
                        d.range,
                    );
                    from_tracks(&tracks, &tags, d.range).unwrap().active_ms
                },
                d.active_ms
            );
        }
        assert_eq!(
            data.report
                .rows(GroupBy::Tag, None)
                .iter()
                .map(|r| r.duration_ms)
                .sum::<i64>(),
            180_000
        );
    }
}

#[cfg(test)]
mod monthly_tests {
    use super::*;
    #[test]
    fn month_report_equals_daily_timesheet_and_clips_month_boundary() {
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let mut date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let first = date;
        for index in 0..31 {
            let range = model::day_bounds(date);
            let start = range.start + 9 * 3_600_000;
            let mut db = shared.storage.lock().unwrap();
            db.record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                start,
                start + 60_000,
            )
            .unwrap();
            db.record_interval(
                "window",
                &serde_json::json!({"app":"editor","title":"work"}),
                start,
                start + 60_000,
            )
            .unwrap();
            if index % 2 == 0 {
                db.tag_interval("Client,Task", "", start, start + 60_000, false)
                    .unwrap();
            }
            drop(db);
            date = date.succ_opt().unwrap();
        }
        let last = model::day_bounds(date).start;
        shared
            .storage
            .lock()
            .unwrap()
            .record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                last - 30_000,
                last + 30_000,
            )
            .unwrap();
        let data = load(
            shared.clone(),
            first.to_string(),
            "2026-10-31".into(),
            false,
        )
        .unwrap();
        assert_eq!(data.days.len(), 31);
        assert_eq!(data.report.active_ms, 31 * 60_000 + 30_000);
        assert_eq!(
            data.report.active_ms,
            data.days.iter().map(|d| d.active_ms).sum::<i64>()
        );
        assert_eq!(data.report.to, last);
        assert!(data.report.intervals.iter().all(|i| i.end_ts <= last));
        for row in data.report.rows(GroupBy::Tag, None) {
            assert_eq!(
                row.duration_ms,
                data.days
                    .iter()
                    .map(|d| d.tag_totals.get(&row.tag_id).copied().unwrap_or(0))
                    .sum::<i64>()
            );
        }
    }
}

#[cfg(test)]
mod memory_regression_tests {
    use super::*;
    #[test]
    fn daily_totals_split_intervals_without_copying_history() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let first = model::day_bounds(date);
        let second = model::day_bounds(date.succ_opt().unwrap());
        let report = Report {
            from: first.start,
            to: second.end,
            active_ms: 180_000,
            intervals: vec![draugr_time_core::ReportInterval {
                start_ts: first.end - 60_000,
                end_ts: first.end + 120_000,
                tag_id: Some(1),
                tag: "Work".into(),
                application: "Editor".into(),
                document: "draft".into(),
            }],
        };
        let a = Day::summarize(date, first, &report);
        let b = Day::summarize(date.succ_opt().unwrap(), second, &report);
        assert_eq!(a.active_ms, 60_000);
        assert_eq!(b.active_ms, 120_000);
        assert_eq!(
            a.tag_totals[&Some(1)] + b.tag_totals[&Some(1)],
            report.active_ms
        );
    }
    #[test]
    fn minute_bands_are_bounded_and_cache_rebuilds_only_on_resize_or_theme_change() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let range = model::day_bounds(date);
        let tracks = model::fixture(range);
        let report = from_tracks(&tracks, &[], range).unwrap();
        let mut day = Day::summarize(date, range, &report);
        day.bands = bands_from_tracks(&tracks, &report, range, &[]);
        assert!(day.bands.iter().map(Vec::len).sum::<usize>() <= 3 * 1440);
        let stats = timeline::PaintStats::default();
        let canvas = WeekCanvas {
            day: &day,
            stats: &stats,
        };
        let renderer = Renderer::new(iced::Font::DEFAULT, 16.0.into());
        use canvas::Program;
        let bounds = Rectangle::with_size(Size::new(800.0, 52.0));
        for _ in 0..2 {
            canvas.draw(
                &(),
                &renderer,
                &native_theme(false),
                bounds,
                mouse::Cursor::Unavailable,
            );
        }
        assert_eq!(stats.frames.load(Ordering::Relaxed), 1);
        canvas.draw(
            &(),
            &renderer,
            &native_theme(true),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 2);
        canvas.draw(
            &(),
            &renderer,
            &native_theme(true),
            Rectangle::with_size(Size::new(900.0, 52.0)),
            mouse::Cursor::Unavailable,
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 3);
    }
}

#[cfg(test)]
mod band_boundary_tests {
    use super::*;
    #[test]
    fn minute_cells_keep_gaps_and_subminute_edges_on_dst_length_days() {
        for hours in [23, 25] {
            let day = Range::new(1000, 1000 + hours * 3_600_000);
            let mut cells = [None; 1440];
            let active = [0.37, 0.65, 0.58];
            let away = [0.83, 0.69, 0.40];
            fill_cells(
                &mut cells,
                Range::new(day.start, day.start + 1),
                active,
                day,
            );
            fill_cells(&mut cells, Range::new(day.end - 1, day.end), away, day);
            assert_eq!(cells[0], Some(active));
            assert_eq!(cells[1439], Some(away));
            assert!(cells[1..1439].iter().all(Option::is_none));
            let bands = compress_cells([cells, [None; 1440], [None; 1440]]);
            assert_eq!(bands[0].len(), 2);
        }
    }
}
