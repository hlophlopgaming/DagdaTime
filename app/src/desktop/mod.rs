//! Native Draugr Time desktop.
mod appearance;
mod model;
mod runtime;
mod single_instance;
mod startup;
mod tagging;
mod timeline;
mod tray;

use base64::Engine;
use chrono::{Days, Local, NaiveDate};
use iced::{
    keyboard,
    widget::{
        self, button, checkbox, column, container, image, row, scrollable, slider, space, svg,
        text, text_input, tooltip,
    },
    window, Alignment, Color, Element, Length, Subscription, Task, Theme,
};
use model::{Range, Summary, Track};
use runtime::{Runtime, Snapshot};
use serde_json::Value;
use startup::StartupProvider;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

static TEST_PASSED: AtomicBool = AtomicBool::new(false);

fn decode_icons(encoded_icons: HashMap<String, Option<String>>) -> HashMap<String, AppIcon> {
    encoded_icons
        .into_iter()
        .filter_map(|(app, encoded)| {
            let encoded = encoded?;
            let (mime, bytes) = encoded.split_once(',').and_then(|(mime, data)| {
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .ok()
                    .map(|bytes| (mime, bytes))
            })?;
            let icon = if mime.contains("svg") {
                AppIcon::Svg(svg::Handle::from_memory(bytes))
            } else {
                AppIcon::Png(image::Handle::from_bytes(bytes))
            };
            Some((app, icon))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub enum Message {
    Edit(tagging::Edit),
    Tick,
    Tray(tray::Event),
    Opened(window::Id),
    Close(window::Id),
    Screen(&'static str),
    Date(String),
    Shift(i32),
    Today,
    Search(String),
    Tab(usize),
    Select(Option<Range>),
    Hover(Option<(usize, usize)>),
    Highlight(String),
    Zoom(f64, Option<i64>),
    Fit,
    Pan(f64),
    PanBy(f64),
    Tracks,
    Visible(usize, bool),
    Move(usize, i32),
    More,
    Theme,
    #[cfg(not(target_os = "linux"))]
    SystemThemeChanged(iced::theme::Mode),
    #[cfg(target_os = "linux")]
    PortalThemeChanged(Option<iced::theme::Mode>),
    #[cfg(target_os = "linux")]
    RefreshSystemTheme,
    Start,
    AutoStart(bool),
    Pause(Option<i64>),
    Resume,
    Quit,
    Folder,
    FolderChosen(Option<std::path::PathBuf>),
    Key(keyboard::Key, keyboard::Modifiers),
    Loaded(u64, Result<Snapshot, String>),
    Screenshot(window::Screenshot),
}
#[derive(Clone)]
pub(super) enum AppIcon {
    Png(image::Handle),
    Svg(svg::Handle),
}
struct State {
    tags: Vec<Value>,
    tag_ranges: Vec<Value>,
    editor: Option<tagging::Editor>,
    tag_undo: Vec<draugr_time_core::TagUndo>,
    away_prompt: tagging::AwayPrompt,
    away_range: Option<Range>,
    date: NaiveDate,
    date_text: String,
    day: Range,
    view: Range,
    all: Vec<Track>,
    filtered: Vec<Vec<usize>>,
    search_text: Vec<Vec<String>>,
    order: Vec<usize>,
    visible: Vec<usize>,
    shown: [bool; 4],
    selection: Option<Range>,
    hover: Option<(usize, usize)>,
    highlight: Option<String>,
    search: String,
    screen: &'static str,
    tab: usize,
    rows: Vec<Summary>,
    documents: Vec<Summary>,
    limit: usize,
    metrics: [i64; 4],
    fixture: bool,
    tracks_menu: bool,
    theme: Theme,
    theme_preference: appearance::Preference,
    system_theme: iced::theme::Mode,
    grid_cache: widget::canvas::Cache,
    cache: widget::canvas::Cache,
    stats: Arc<timeline::PaintStats>,
    icons: HashMap<String, AppIcon>,
    preview_apps: Vec<(String, String)>,
    runtime: Option<Runtime>,
    tray: Option<tray::Tray>,
    status: Value,
    error: String,
    loading: bool,
    /// Bumped whenever the shown day or data source changes; older loads are dropped.
    load_generation: u64,
    load_cursor: runtime::LoadCursor,
    /// Day whose recorded history is currently shown; past days are loaded once.
    loaded_day: Option<Range>,
    clock: Instant,
    last_sample: Instant,
    last_load: Instant,
    window: Option<window::Id>,
    smoke: bool,
    runtime_test: bool,
    lifecycle: bool,
    cycle_stage: u8,
    cycle_duration: i64,
    cycle_frames: usize,
    benchmark: bool,
    snapshot_done: bool,
    closed_for_benchmark: bool,
    pending_dir: Option<String>,
    autostart_enabled: bool,
    autostart_error: String,
}
impl State {
    fn boot() -> (Self, Task<Message>) {
        let args: Vec<_> = std::env::args().collect();
        let launch = startup::LaunchOptions::parse(args.iter().map(String::as_str));
        let smoke = args.iter().any(|arg| arg == "--smoke-test");
        let lifecycle = args.iter().any(|arg| arg == "--lifecycle-test");
        let runtime_test = lifecycle || args.iter().any(|arg| arg == "--runtime-test");
        let benchmark = args.iter().any(|arg| arg == "--benchmark");
        let date = Local::now().date_naive();
        let day = model::day_bounds(date);
        let stats = Arc::new(timeline::PaintStats::default());
        let preview_apps = if cfg!(test) || !(smoke || benchmark) {
            Vec::new()
        } else {
            crate::icons::preview_application_processes()
        };
        let icons = decode_icons(crate::icons::application_icons(preview_apps.clone()));
        let tray = if cfg!(test) || smoke || (runtime_test && !lifecycle) {
            None
        } else {
            match tray::Tray::new() {
                Ok(tray) => Some(tray),
                Err(error) => {
                    eprintln!("Tray unavailable: {error}");
                    None
                }
            }
        };
        let mut state = Self {
            tags: Vec::new(),
            tag_ranges: Vec::new(),
            editor: None,
            tag_undo: Vec::new(),
            away_prompt: tagging::AwayPrompt {
                threshold: tagging::load_threshold(),
                seen_end: draugr_time_daemon::now_ms(),
            },
            away_range: None,
            date,
            date_text: date.to_string(),
            day,
            view: day,
            all: if smoke || benchmark {
                model::fixture_with_apps(day, &preview_apps)
            } else {
                model::empty_tracks()
            },
            filtered: Vec::new(),
            search_text: Vec::new(),
            order: vec![0, 1, 2, 3],
            visible: vec![0, 1, 2, 3],
            shown: [true; 4],
            selection: None,
            hover: None,
            highlight: None,
            documents: Vec::new(),
            search: String::new(),
            screen: "Day",
            tab: 1,
            rows: Vec::new(),
            limit: 50,
            metrics: [0; 4],
            fixture: smoke || benchmark,
            tracks_menu: false,
            theme: native_theme(false),
            theme_preference: appearance::Preference::System,
            system_theme: iced::theme::Mode::None,
            grid_cache: widget::canvas::Cache::new(),
            cache: widget::canvas::Cache::new(),
            stats,
            icons,
            preview_apps,
            runtime: None,
            tray,
            status: Value::Null,
            error: String::new(),
            loading: false,
            load_generation: 0,
            load_cursor: runtime::LoadCursor::default(),
            loaded_day: None,
            clock: Instant::now(),
            last_sample: Instant::now() - Duration::from_secs(2),
            last_load: Instant::now() - Duration::from_secs(5),
            window: None,
            smoke,
            runtime_test,
            lifecycle,
            cycle_stage: 0,
            cycle_duration: 0,
            cycle_frames: 0,
            benchmark,
            snapshot_done: false,
            closed_for_benchmark: false,
            pending_dir: None,
            autostart_enabled: startup::SystemStartup::new()
                .and_then(|startup| startup.is_enabled())
                .unwrap_or(false),
            autostart_error: String::new(),
        };
        if runtime_test || launch.record_on_launch {
            state.start();
        }
        state.rebuild();
        let task = state.open(!launch.hidden || state.tray.is_none());
        #[cfg(target_os = "linux")]
        let system_theme = Task::perform(
            async {
                tokio::task::spawn_blocking(appearance::system_mode)
                    .await
                    .ok()
                    .flatten()
            },
            Message::PortalThemeChanged,
        );
        #[cfg(not(target_os = "linux"))]
        let system_theme = iced::system::theme().map(Message::SystemThemeChanged);
        (state, Task::batch([task, system_theme]))
    }
    fn apply_theme(&mut self) {
        self.theme = native_theme(self.theme_preference.is_dark(self.system_theme));
        self.cache.clear();
        self.grid_cache.clear();
    }
    fn open(&mut self, visible: bool) -> Task<Message> {
        if let Some(id) = self.window {
            return window::gain_focus(id);
        }
        let (id, task) = window::open(window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            visible,
            exit_on_close_request: false,
            ..window::Settings::default()
        });
        self.window = Some(id);
        task.map(Message::Opened)
    }
    fn start(&mut self) {
        if self.runtime.is_some() {
            self.fixture = false;
            self.all = model::empty_tracks();
            self.invalidate_load();
            self.rebuild();
            return;
        }
        match Runtime::start(self.runtime_test) {
            Ok(runtime) => {
                self.runtime = Some(runtime);
                self.fixture = false;
                self.all = model::empty_tracks();
                self.error.clear();
                self.invalidate_load();
            }
            Err(error) => {
                self.error = error;
            }
        }
        self.rebuild();
    }
    /// Request a fresh load and discard any load still in flight: its result belongs
    /// to a different day or data source and must not replace what is shown.
    fn invalidate_load(&mut self) {
        self.load_generation = self.load_generation.wrapping_add(1);
        self.loading = false;
        self.loaded_day = None;
        self.load_cursor = runtime::LoadCursor::default();
        self.search_text.clear();
        self.last_load = Instant::now() - Duration::from_secs(5);
    }
    fn rebuild(&mut self) {
        let ranges = |track: usize, key: Option<&str>| {
            self.all[track]
                .segments
                .iter()
                .filter(|s| key.is_none_or(|key| s.key.as_ref() == key))
                .map(|s| s.range)
                .collect::<Vec<_>>()
        };
        self.metrics = [
            model::union_duration(&ranges(1, Some("active")), self.day),
            model::union_duration(&ranges(1, Some("away")), self.day),
            model::union_duration(
                &self.all[0]
                    .segments
                    .iter()
                    .filter(|s| s.key.as_ref() != "untagged")
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                self.day,
            ),
            model::union_duration(&ranges(0, Some("untagged")), self.day),
        ];
        // Refresh the precomputed search text only for changed/new segment strings.
        self.search_text.resize_with(self.all.len(), Vec::new);
        for (text, track) in self.search_text.iter_mut().zip(&self.all) {
            text.truncate(track.segments.len());
            for segment in track.segments.iter().skip(text.len()) {
                text.push(
                    format!("{} {} {}", segment.label, segment.detail, segment.key).to_lowercase(),
                );
            }
        }
        let search = self.search.to_lowercase();
        self.filtered = self
            .all
            .iter()
            .enumerate()
            .map(|(track, values)| {
                (0..values.segments.len())
                    .filter(|index| {
                        search.trim().is_empty()
                            || self.search_text[track][*index].contains(&search)
                    })
                    .collect()
            })
            .collect();
        let old_visible = self.visible.clone();
        self.visible = self
            .order
            .iter()
            .copied()
            .filter(|index| self.shown[*index])
            .collect();
        if self.visible != old_visible {
            self.grid_cache.clear();
        }
        self.summary();
        self.hover = None;
        self.cache.clear();
    }
    fn summary(&mut self) {
        let track = [0, 2, 3][self.tab];
        self.rows = model::summarize_iter(
            self.filtered[track]
                .iter()
                .map(|i| &self.all[track].segments[*i]),
            self.selection.unwrap_or(self.day),
        );
        self.documents = if self.tab == 1 {
            self.highlight
                .as_deref()
                .map(|key| {
                    model::application_documents_iter(
                        self.filtered[3].iter().map(|i| &self.all[3].segments[*i]),
                        key,
                        self.selection.unwrap_or(self.day),
                    )
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    }
    fn change_date(&mut self, date: NaiveDate) {
        self.date = date;
        self.date_text = date.to_string();
        self.day = model::day_bounds(date);
        self.view = self.day;
        self.grid_cache.clear();
        self.selection = None;
        self.highlight = None;
        self.limit = 50;
        if self.fixture {
            self.search_text.clear();
            self.all = model::fixture_with_apps(self.day, &self.preview_apps);
        } else {
            self.all = model::empty_tracks();
            self.invalidate_load();
        }
        self.rebuild();
    }
    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            Subscription::run(tray::events).map(Message::Tray),
            window::close_requests().map(Message::Close),
            iced::event::listen_with(|event, status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                    if status == iced::event::Status::Ignored
                        || (modifiers.command()
                            && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("f"))) =>
                {
                    Some(Message::Key(key, modifiers))
                }
                _ => None,
            }),
        ];
        if self.window.is_some() || self.lifecycle || self.benchmark || cfg!(target_os = "macos") {
            subscriptions.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        // Polling the portal only matters while there is a window to theme.
        #[cfg(target_os = "linux")]
        if self.theme_preference == appearance::Preference::System && self.window.is_some() {
            subscriptions.push(
                iced::time::every(Duration::from_secs(2)).map(|_| Message::RefreshSystemTheme),
            );
        }
        #[cfg(not(target_os = "linux"))]
        subscriptions.push(iced::system::theme_changes().map(Message::SystemThemeChanged));
        Subscription::batch(subscriptions)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(event) => return self.edit(event),
            Message::Tray(tray::Event::Menu(event)) => {
                let message = match event.as_str() {
                    "open" => return self.open(true),
                    "settings" => {
                        self.screen = "Settings";
                        return self.open(true);
                    }
                    "start" => Message::Start,
                    "pause" => Message::Pause(None),
                    "pause15" => Message::Pause(Some(900_000)),
                    "resume" => Message::Resume,
                    "quick5" => Message::Edit(tagging::Edit::Quick(5)),
                    "quick15" => Message::Edit(tagging::Edit::Quick(15)),
                    "quick60" => Message::Edit(tagging::Edit::Quick(60)),
                    "quit" => Message::Quit,
                    _ => return Task::none(),
                };
                return self.update(message);
            }
            Message::Tray(tray::Event::Status(status)) => {
                let returned = self.status["computer"]["activity"] != "active"
                    && status["computer"]["activity"] == "active";
                self.status = status;
                if returned && self.observe_away() {
                    return self.open(true);
                }
                if let Some(tray) = &self.tray {
                    tray.update(&self.status, self.metrics[0], self.fixture);
                }
            }
            Message::Tick => {
                if self.last_sample.elapsed() >= Duration::from_secs(1) {
                    let was_active = self.status["computer"]["activity"] == "active";
                    self.last_sample = Instant::now();
                    if let Some(runtime) = self.runtime.as_mut() {
                        #[cfg(target_os = "macos")]
                        runtime.pulse();
                        self.status = runtime.status();
                    }
                    if !was_active
                        && self.status["computer"]["activity"] == "active"
                        && self.observe_away()
                    {
                        return self.open(true);
                    }
                    if let Some(tray) = self.tray.as_ref() {
                        tray.update(&self.status, self.metrics[0], self.fixture);
                    }
                }
                let painted = self.stats.frames.load(Ordering::Relaxed) > 0;
                if self.lifecycle && painted {
                    let recorded = self.all[2]
                        .segments
                        .iter()
                        .map(|s| s.range.duration())
                        .sum::<i64>();
                    if self.cycle_stage == 0
                        && self.clock.elapsed() > Duration::from_secs(3)
                        && recorded > 0
                    {
                        self.cycle_duration = recorded;
                        self.cycle_stage = 1;
                        if let Some(id) = self.window {
                            return self.update(Message::Close(id));
                        }
                    }
                    if self.cycle_stage == 1 && recorded >= self.cycle_duration + 2_000 {
                        self.cycle_stage = 2;
                        self.cycle_frames = self.stats.frames.load(Ordering::Relaxed);
                        return self.open(true);
                    }
                    if self.cycle_stage == 2
                        && self.stats.frames.load(Ordering::Relaxed) > self.cycle_frames
                        && self.clock.elapsed() > Duration::from_secs(8)
                    {
                        TEST_PASSED.store(true, Ordering::Relaxed);
                        eprintln!("Iced lifecycle passed: tray created, window closed, recording continued, window reopened");
                        return iced::exit();
                    }
                }
                if self.smoke
                    && !TEST_PASSED.load(Ordering::Relaxed)
                    && painted
                    && self.clock.elapsed() > Duration::from_secs(2)
                {
                    TEST_PASSED.store(true, Ordering::Relaxed);
                    eprintln!(
                        "Iced smoke passed: 5000 fixture events, canvas build max {:.2} ms",
                        self.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                    );
                    return iced::exit();
                }
                if self.runtime_test
                    && !self.lifecycle
                    && painted
                    && self.all[2].segments.iter().any(|s| s.range.duration() > 0)
                {
                    TEST_PASSED.store(true, Ordering::Relaxed);
                    eprintln!("Iced runtime passed: native window and embedded SQLite recording");
                    return iced::exit();
                }
                if (self.smoke || self.runtime_test)
                    && self.clock.elapsed() > Duration::from_secs(20)
                {
                    eprintln!("Iced smoke/runtime check timed out");
                    std::process::exit(1);
                }
                if self.benchmark
                    && painted
                    && !self.snapshot_done
                    && self.clock.elapsed() > Duration::from_secs(3)
                {
                    self.snapshot_done = true;
                    if let Some(id) = self.window {
                        return window::screenshot(id).map(Message::Screenshot);
                    }
                }
                if self.benchmark
                    && !self.closed_for_benchmark
                    && self.clock.elapsed() > Duration::from_secs(7)
                {
                    self.closed_for_benchmark = true;
                    if let Some(id) = self.window.take() {
                        self.cache.clear();
                        self.grid_cache.clear();
                        return window::close(id);
                    }
                }
                if self.benchmark && self.clock.elapsed() > Duration::from_secs(12) {
                    eprintln!(
                        "Iced benchmark: canvas build max {:.2} ms",
                        self.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                    );
                    return iced::exit();
                }
                let now = draugr_time_daemon::now_ms();
                let today = self.day.start <= now && now < self.day.end;
                // Only today's history still changes; a past day is loaded once.
                let stale = self.loaded_day != Some(self.day)
                    || (today && self.last_load.elapsed() > Duration::from_secs(2));
                if !self.fixture
                    && !self.loading
                    && stale
                    && (self.window.is_some() || self.lifecycle)
                {
                    if let Some(runtime) = self.runtime.as_ref() {
                        let shared = runtime.shared.clone();
                        let day = self.day;
                        let cursor = self.load_cursor.clone();
                        let generation = self.load_generation;
                        self.loading = true;
                        self.last_load = Instant::now();
                        return Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    Runtime::load(shared, day, cursor)
                                })
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r)
                            },
                            move |result| Message::Loaded(generation, result),
                        );
                    }
                }
            }
            Message::Loaded(generation, result) => {
                if generation != self.load_generation {
                    // Started before a date or recording change; not for this view.
                    return Task::none();
                }
                self.loading = false;
                match result {
                    Ok(snapshot) => {
                        if self.away_range.is_none() {
                            self.away_range = self.away_prompt.observe(snapshot.completed_afk);
                        }
                        if let Some(metadata) = snapshot.tag_metadata {
                            self.tags = metadata.tags;
                            self.tag_ranges = metadata.ranges;
                        }
                        self.loaded_day = Some(self.day);
                        let new_icons: HashMap<_, _> = snapshot
                            .icons
                            .into_iter()
                            .filter(|(key, _)| !self.icons.contains_key(key))
                            .collect();
                        self.icons.extend(decode_icons(new_icons));
                        if self.editor.is_none() {
                            self.error.clear();
                        }
                        self.load_cursor = snapshot.cursor;
                        let mut changed = false;
                        for (index, (track, incoming)) in
                            self.all.iter_mut().zip(snapshot.tracks).enumerate()
                        {
                            if index == 0 {
                                if snapshot.tags_changed {
                                    track.segments.retain(|s| s.bucket == "derived");
                                    track.segments.extend(incoming.segments);
                                    self.search_text.clear();
                                    changed = true;
                                }
                                continue;
                            }
                            let split = track
                                .segments
                                .iter()
                                .position(|s| {
                                    snapshot
                                        .replace_from
                                        .get(s.bucket)
                                        .is_some_and(|from| s.range.start >= *from)
                                })
                                .unwrap_or(track.segments.len());
                            let mut tail: Vec<_> = track.segments[split..]
                                .iter()
                                .filter(|s| {
                                    s.bucket != "derived"
                                        && snapshot
                                            .replace_from
                                            .get(s.bucket)
                                            .is_none_or(|from| s.range.start < *from)
                                })
                                .cloned()
                                .collect();
                            tail.extend(incoming.segments);
                            tail.sort_by_key(|s| (s.range.start, s.range.end));
                            // Compare only the fetched tail; history is never cloned or compared.
                            let old: Vec<_> = track.segments[split..]
                                .iter()
                                .filter(|s| s.bucket != "derived")
                                .collect();
                            if !old.iter().copied().eq(tail.iter()) {
                                track.segments.truncate(split);
                                track.segments.extend(tail);
                                if let Some(text) = self.search_text.get_mut(index) {
                                    text.truncate(split);
                                }
                                changed = true;
                            }
                        }
                        if changed {
                            model::derive_tracks(&mut self.all, self.day);
                            // Derived Tags and Computer intervals can move when gaps close.
                            for index in [0, 1] {
                                if let Some(text) = self.search_text.get_mut(index) {
                                    text.clear();
                                }
                            }
                            self.rebuild();
                        }
                    }
                    Err(error) => self.error = error,
                }
            }
            Message::Opened(id) => self.window = Some(id),
            Message::Close(id) => {
                if self.tray.is_some() {
                    self.window = None;
                    self.cache.clear();
                    self.grid_cache.clear();
                    return window::close(id);
                }
                return iced::exit();
            }
            Message::Screen(screen) => self.screen = screen,
            Message::Date(input) => {
                self.date_text = input.clone();
                if let Ok(date) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
                    self.change_date(date);
                }
            }
            Message::Shift(delta) => {
                let date = if delta < 0 {
                    self.date.checked_sub_days(Days::new(1))
                } else {
                    self.date.checked_add_days(Days::new(1))
                };
                if let Some(date) = date {
                    self.change_date(date);
                }
            }
            Message::Today => self.change_date(Local::now().date_naive()),
            Message::Search(search) => {
                self.search = search;
                self.limit = 50;
                self.rebuild();
            }
            Message::Tab(tab) => {
                self.tab = tab;
                self.highlight = None;
                self.limit = 50;
                self.cache.clear();
                self.grid_cache.clear();
                self.summary();
            }
            Message::Select(selection) => {
                self.selection = selection;
                self.highlight = None;
                self.limit = 50;
                self.summary();
            }
            Message::Hover(hover) => self.hover = hover,
            Message::Highlight(key) => {
                self.highlight = if self.highlight.as_ref() == Some(&key) {
                    None
                } else {
                    Some(key)
                };
                self.summary();
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::Zoom(factor, anchor) => {
                let anchor = anchor.or_else(|| self.selection.map(|s| s.start + s.duration() / 2));
                self.view = model::zoom(self.day, self.view, factor, anchor);
                self.hover = None;
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::Fit => {
                self.view = self.day;
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::Pan(value) => {
                let span = self.view.duration();
                let start = self.day.start
                    + (value.clamp(0.0, 1.0) * (self.day.duration() - span) as f64).round() as i64;
                self.view = Range::new(start, start + span);
                self.cache.clear();
                self.grid_cache.clear();
                self.hover = None;
            }
            Message::PanBy(value) => {
                let span = self.view.duration();
                let start = (self.view.start + (value * span as f64) as i64)
                    .clamp(self.day.start, self.day.end - span);
                self.view = Range::new(start, start + span);
                self.cache.clear();
                self.grid_cache.clear();
                self.hover = None;
            }
            Message::Tracks => self.tracks_menu = !self.tracks_menu,
            Message::Visible(index, visible) => {
                self.shown[index] = visible;
                self.rebuild();
            }
            Message::Move(index, delta) => {
                let next = (index as i32 + delta) as usize;
                if next < self.order.len() {
                    self.order.swap(index, next);
                    self.rebuild();
                }
            }
            Message::More => self.limit += 50,
            Message::Theme => {
                self.theme_preference = self.theme_preference.next();
                self.apply_theme();
            }
            #[cfg(not(target_os = "linux"))]
            Message::SystemThemeChanged(mode) => {
                let changed = self.system_theme != mode;
                self.system_theme = mode;
                if changed && self.theme_preference == appearance::Preference::System {
                    self.apply_theme();
                }
            }
            #[cfg(target_os = "linux")]
            Message::PortalThemeChanged(Some(mode)) => {
                let changed = self.system_theme != mode;
                self.system_theme = mode;
                if changed && self.theme_preference == appearance::Preference::System {
                    self.apply_theme();
                }
            }
            #[cfg(target_os = "linux")]
            Message::PortalThemeChanged(None) => {}
            #[cfg(target_os = "linux")]
            Message::RefreshSystemTheme => {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(appearance::system_mode)
                            .await
                            .ok()
                            .flatten()
                    },
                    Message::PortalThemeChanged,
                );
            }
            Message::Start => self.start(),
            Message::AutoStart(enabled) => match startup::SystemStartup::new()
                .and_then(|mut provider| startup::set_enabled(&mut provider, enabled))
            {
                Ok(enabled) => {
                    self.autostart_enabled = enabled;
                    self.autostart_error.clear();
                }
                Err(error) => self.autostart_error = error,
            },
            Message::Pause(duration) => {
                if let Some(runtime) = self.runtime.as_ref() {
                    if let Err(error) = runtime.pause(duration) {
                        self.error = error;
                    }
                    self.status = runtime.status();
                }
            }
            Message::Resume => {
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime.resume();
                    self.status = runtime.status();
                }
            }
            Message::Quit => return iced::exit(),
            Message::Folder => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(|folder| folder.path().to_owned())
                    },
                    Message::FolderChosen,
                )
            }
            Message::FolderChosen(folder) => {
                if let Some(folder) = folder {
                    let result = draugr_time_daemon::paths::default_dir().and_then(|dir| {
                        draugr_time_daemon::paths::save_choice(
                            &dir.join("preferences.json"),
                            &folder,
                        )
                    });
                    match result {
                        Ok(()) => self.pending_dir = Some(folder.to_string_lossy().into_owned()),
                        Err(error) => self.error = error.to_string(),
                    }
                }
            }
            Message::Key(key, modifiers) => {
                if modifiers.command()
                    && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("z"))
                {
                    return self.edit(tagging::Edit::Undo);
                }
                if self.editor.is_some() {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            self.edit(tagging::Edit::Cancel)
                        }
                        keyboard::Key::Named(keyboard::key::Named::Enter)
                            if self.editor.as_ref().is_some_and(|e| !e.overlap) =>
                        {
                            self.edit(
                                if self.editor.as_ref().is_some_and(|e| e.tag_id.is_some()) {
                                    tagging::Edit::SaveTag
                                } else {
                                    tagging::Edit::Save(false)
                                },
                            )
                        }
                        _ => Task::none(),
                    };
                }
                if key == keyboard::Key::Named(keyboard::key::Named::Enter) {
                    return self.edit(tagging::Edit::Open);
                }
                if modifiers.command()
                    && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("f"))
                {
                    return widget::operation::focus("activity-search");
                }
                let message = match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        Some(Message::Shift(-1))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        Some(Message::Shift(1))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Some(Message::Select(None))
                    }
                    keyboard::Key::Character(c) => match c.as_str() {
                        "t" | "T" => Some(Message::Today),
                        "+" | "=" => Some(Message::Zoom(0.9, None)),
                        "-" => Some(Message::Zoom(1.0 / 0.9, None)),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(message) = message {
                    return self.update(message);
                }
            }
            Message::Screenshot(screenshot) => {
                if let Some(path) = std::env::var_os("DRAUGR_TIME_SCREENSHOT") {
                    let result = (|| -> Result<(), String> {
                        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
                        let mut encoder =
                            png::Encoder::new(file, screenshot.size.width, screenshot.size.height);
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder
                            .write_header()
                            .map_err(|e| e.to_string())?
                            .write_image_data(&screenshot.rgba)
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        eprintln!("Screenshot failed: {error}");
                    }
                }
            }
        }
        Task::none()
    }
    fn status_label(&self) -> &'static str {
        match self.status["state"].as_str() {
            Some("tracking") => "● Tracking",
            Some("paused") => "● Paused",
            Some("away") => "● AFK",
            Some("asleep") => "● Sleep",
            Some("locked") => "● Locked",
            Some("error") => "● Needs attention",
            Some("starting") => "● Starting",
            _ => "○ Not recording",
        }
    }
    fn app_icon<'a>(&'a self, app: &'a str, app_name: &'a str) -> Element<'a, Message> {
        let icon_key = model::application_identity(app, app_name);
        match self.icons.get(&icon_key) {
            Some(AppIcon::Png(handle)) => image(handle.clone()).width(20).height(20).into(),
            Some(AppIcon::Svg(handle)) => svg(handle.clone()).width(20).height(20).into(),
            None => {
                let rgb = model::color(app_name);
                container(
                    text(
                        app_name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string(),
                    )
                    .size(11),
                )
                .width(20)
                .height(20)
                .center(20)
                .style(move |_: &Theme| container::Style {
                    background: Some(Color::from_rgb(rgb[0], rgb[1], rgb[2]).into()),
                    text_color: Some(Color::WHITE),
                    border: iced::Border::default().rounded(4),
                    ..container::Style::default()
                })
                .into()
            }
        }
    }
    fn day_view(&self) -> Element<'_, Message> {
        let mut metrics = row![].spacing(10);
        for (label, value) in [
            "ACTIVE TIME",
            "AWAY TIME",
            "TAGGED TIME",
            "UNTAGGED ACTIVITY",
        ]
        .into_iter()
        .zip(self.metrics)
        {
            metrics = metrics.push(
                container(
                    column![text(label).size(10), text(model::duration(value)).size(24)].spacing(2),
                )
                .width(Length::Fill)
                .padding(10)
                .style(panel),
            );
        }
        let mode = if self.fixture {
            "Preview · 5,000 generated events · sample data is never saved"
        } else {
            "Recorded activity"
        };
        let heading = row![
            text("Activity timeline").size(15),
            space::horizontal(),
            button("Tracks").on_press(Message::Tracks),
            button("−").on_press(Message::Zoom(1.0 / 0.9, None)),
            text(if self.view == self.day {
                "Full day".into()
            } else {
                model::duration(self.view.duration())
            })
            .size(12),
            button("+").on_press(Message::Zoom(0.9, None)),
            button("Fit day").on_press(Message::Fit)
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut timeline_body = column![heading].spacing(8);
        if self.tracks_menu {
            let mut menu = column![].spacing(4);
            for (index, track) in self.order.iter().enumerate() {
                let up = (index > 0).then_some(Message::Move(index, -1));
                let down = (index + 1 < self.order.len()).then_some(Message::Move(index, 1));
                menu = menu.push(
                    row![
                        checkbox(self.shown[*track])
                            .label(self.all[*track].name)
                            .on_toggle(move |value| Message::Visible(*track, value)),
                        space::horizontal(),
                        button("↑").on_press_maybe(up),
                        button("↓").on_press_maybe(down)
                    ]
                    .spacing(8),
                );
            }
            timeline_body = timeline_body.push(menu);
        }
        let labels = self.visible.iter().fold(
            column![container(text("LOCAL TIME").size(10))
                .height(timeline::RULER)
                .center_y(timeline::RULER)],
            |column, index| {
                column.push(
                    container(text(self.all[*index].name).size(12))
                        .height(timeline::ROW)
                        .center_y(timeline::ROW),
                )
            },
        );
        let plot = timeline::canvas(timeline::Timeline {
            tracks: &self.all,
            filtered: &self.filtered,
            visible: &self.visible,
            view: self.view,
            selection: self.selection,
            highlight: self.highlight.as_deref(),
            grid_cache: &self.grid_cache,
            cache: &self.cache,
            stats: &self.stats,
            icons: &self.icons,
        });
        let chart: Element<'_, Message> = if let Some(segment) = self
            .hover
            .and_then(|(track, index)| self.all.get(track)?.segments.get(index))
        {
            let details = column![
                text(segment.label.as_ref()).size(14),
                text(segment.detail.as_ref()).size(12),
                text(format!(
                    "{}–{} · {}",
                    model::time_label(segment.range.start),
                    model::time_label(segment.range.end),
                    model::duration(segment.range.duration())
                ))
                .size(12)
            ]
            .spacing(5);
            let details: Element<'_, Message> = if let (Some(app), Some(app_name)) =
                (segment.app.as_deref(), segment.app_name.as_deref())
            {
                row![self.app_icon(app, app_name), details]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into()
            } else {
                details.into()
            };
            tooltip(
                plot,
                container(details).padding(10),
                tooltip::Position::FollowCursor,
            )
            .style(panel)
            .into()
        } else {
            plot.into()
        };
        let pan = (self.view.start - self.day.start) as f64
            / (self.day.duration() - self.view.duration()).max(1) as f64;
        timeline_body=timeline_body.push(row![container(labels).width(124),chart].spacing(8)).push(row![text("Pan across day").size(10).width(124),slider(0.0..=1.0,pan,Message::Pan).step(0.001)].spacing(8)).push(text("Active · AFK · Sleep · Locked · Paused · No recording · Unobserved (diagnostic)   |   Drag to select · Shift-click to extend · Double-click a block · Wheel to zoom").size(10));
        let timeline = container(timeline_body).padding(12).style(panel);
        let selection = self
            .selection
            .map(|s| {
                format!(
                    "Selection {}–{} · {}",
                    model::time_label(s.start),
                    model::time_label(s.end),
                    model::duration(s.duration())
                )
            })
            .unwrap_or_else(|| "Whole day".into());
        let selection_bar = row![
            text(selection).size(14),
            space::horizontal(),
            button("Clear").on_press(Message::Select(None)),
            button("Tag selection ⏎")
                .on_press_maybe(self.selection.map(|_| Message::Edit(tagging::Edit::Open)))
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let tabs =
            ["Tags", "Applications", "Documents"]
                .into_iter()
                .enumerate()
                .fold(row![].spacing(6), |row, (index, label)| {
                    row.push(button(label).on_press(Message::Tab(index)).style(
                        if index == self.tab {
                            button::primary
                        } else {
                            button::secondary
                        },
                    ))
                });
        let total = self.rows.iter().map(|row| row.duration).sum();
        let tabs = row![
            tabs,
            space::horizontal(),
            text(format!("{} observed", model::duration(total))).size(12)
        ]
        .align_y(Alignment::Center);
        let mut body = column![].spacing(3);
        for entry in self.rows.iter().take(self.limit) {
            let rgb = entry.color;
            let icon: Element<'_, Message> = if let (Some(app), Some(app_name)) =
                (entry.app.as_deref(), entry.app_name.as_deref())
            {
                self.app_icon(app, app_name)
            } else {
                container(space::horizontal())
                    .width(10)
                    .height(10)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Color::from_rgb(rgb[0], rgb[1], rgb[2]).into()),
                        ..container::Style::default()
                    })
                    .into()
            };
            let mut label = row![].spacing(8);
            if self.tab == 1 {
                label = label.push(
                    text(if self.highlight.as_deref() == Some(entry.key.as_ref()) {
                        "▾"
                    } else {
                        "▸"
                    })
                    .size(13),
                );
            }
            let label = label
                .push(icon)
                .push(text(entry.label.as_ref()).size(13))
                .spacing(8)
                .align_y(Alignment::Center);
            let style = if self.highlight.as_deref() == Some(entry.key.as_ref()) {
                button::primary
            } else {
                button::text
            };
            body = body.push(
                row![
                    button(label)
                        .on_press(Message::Highlight(entry.key.to_string()))
                        .style(style)
                        .width(Length::Fill),
                    text(model::duration(entry.duration)).size(13).width(90),
                    text(format!("{:.1}%", entry.share)).size(13).width(78)
                ]
                .align_y(Alignment::Center),
            );
            if self.tab == 1 && self.highlight.as_deref() == Some(entry.key.as_ref()) {
                let mut documents =
                    column![text("Documents / pages · most time first").size(11)].spacing(6);
                for document in &self.documents {
                    documents = documents.push(
                        row![
                            text(document.label.as_ref()).size(12).width(Length::Fill),
                            text(model::duration(document.duration)).size(12).width(90),
                            text(format!("{:.1}%", document.share)).size(12).width(78)
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    );
                }
                if self.documents.is_empty() {
                    documents = documents.push(
                        text("No recorded document titles for this application in this range.")
                            .size(12),
                    );
                }
                body = body.push(
                    container(documents)
                        .padding(iced::Padding {
                            top: 4.0,
                            right: 0.0,
                            bottom: 8.0,
                            left: 36.0,
                        })
                        .width(Length::Fill),
                );
            }
        }
        if self.rows.is_empty() {
            body = body.push(
                text(if self.search.is_empty() {
                    "No activity in this range yet."
                } else {
                    "No matching activity."
                })
                .size(13),
            );
        }
        if self.rows.len() > self.limit {
            body = body.push(button("Show 50 more").on_press(Message::More));
        }
        let summary=container(column![selection_bar,tabs,row![text("Project / application / document").size(10).width(Length::Fill),text("Duration").size(10).width(90),text("Range share").size(10).width(78)],scrollable(body).height(Length::Fill),text("Shares refer to the selected time range. Overlaps within one item are counted once.").size(10)].spacing(7)).padding(12).height(Length::Fill).style(panel);
        column![metrics, text(mode).size(12), timeline, summary]
            .spacing(10)
            .height(Length::Fill)
            .into()
    }
    fn settings(&self) -> Element<'_, Message> {
        let mut backend=column![text("Status & permissions").size(22),text("Live collector status. macOS and Wayland still require OS permissions/integration.").size(13)].spacing(12);
        if let Some(computer) = self.status.get("computer") {
            backend = backend.push(
                text(format!(
                    "Computer: {} · Activity: {} · Recording: {}",
                    computer["machine"].as_str().unwrap_or("unknown"),
                    computer["activity"].as_str().unwrap_or("unknown"),
                    computer["recording"].as_str().unwrap_or("unknown")
                ))
                .size(13),
            );
            if let Some(reason) = computer["reason"].as_str() {
                backend = backend.push(text(reason).size(13));
            }
        }
        if let Some(capabilities) = self.status["computer_capabilities"].as_str() {
            backend = backend.push(text(capabilities).size(12));
        }
        if let Some(backends) = self.status["backends"].as_array() {
            for value in backends {
                backend = backend.push(
                    text(format!(
                        "{} · {}",
                        value["name"].as_str().unwrap_or("Collector"),
                        value["state"].as_str().unwrap_or("unknown")
                    ))
                    .size(15),
                );
                if let Some(error) = value["error"].as_str() {
                    backend = backend.push(text(error).size(13));
                }
            }
        } else {
            backend = backend.push(text(
                "Tracking is stopped. Press Start recording to collect activity.",
            ));
        }
        let directory = draugr_time_daemon::paths::data_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|e| e.to_string());
        let folder = column![
            text("Data folder").size(22),
            text(directory).size(13),
            button("Choose folder").on_press(Message::Folder),
            text(
                self.pending_dir
                    .as_ref()
                    .map(|path| format!(
                        "Next launch: {path}. Existing history stays in the old folder."
                    ))
                    .unwrap_or_else(|| {
                        "The folder change takes effect after restarting Draugr Time.".into()
                    })
            )
            .size(12)
        ]
        .spacing(12);
        let autostart = column![
            checkbox(self.autostart_enabled)
                .label("Start Draugr Time when I log in")
                .on_toggle(Message::AutoStart),
            text("Starts activity tracking in the tray. Turn this off here to remove it from system startup.")
                .size(12),
            text(&self.autostart_error)
                .size(12)
                .color(Color::from_rgb8(200, 75, 75))
        ]
        .spacing(8);
        let resource=column![text("Draugr Time").size(22),text("Native Rust UI · tiny-skia software renderer").size(14),text(format!("Canvas draws: {} · max geometry build: {:.2} ms",self.stats.frames.load(Ordering::Relaxed),self.stats.max_build_us.load(Ordering::Relaxed) as f64/1000.0)).size(13),text("Closing the window releases its renderer and keeps recording. Open it again from the tray. Quit stops the process. Themes, tray behavior and OS permissions need verification on each platform.").size(13)].spacing(12);
        scrollable(
            column![
                container(backend).padding(16).style(panel),
                container(folder).padding(16).style(panel),
                container(autostart).padding(16).style(panel),
                container(
                    column![
                        text("While you were away").size(22),
                        text(format!(
                            "Prompt threshold: {}",
                            if self.away_prompt.threshold > 86_400_000 {
                                "disabled".into()
                            } else {
                                model::duration(self.away_prompt.threshold)
                            }
                        )),
                        row![
                            button("15 min").on_press(Message::Edit(tagging::Edit::Threshold(15))),
                            button("30 min").on_press(Message::Edit(tagging::Edit::Threshold(30))),
                            button("60 min").on_press(Message::Edit(tagging::Edit::Threshold(60))),
                            button("Off").on_press(Message::Edit(tagging::Edit::Threshold(
                                i64::MAX / 60_000
                            )))
                        ]
                        .spacing(8)
                    ]
                    .spacing(8)
                )
                .padding(16)
                .style(panel),
                container(resource).padding(16).style(panel)
            ]
            .spacing(14),
        )
        .into()
    }
    fn view(&self, _id: window::Id) -> Element<'_, Message> {
        let mut navigation = column![
            text("◷ Draugr Time").size(23),
            text("LOCAL ACTIVITY TRACKER").size(10),
            space::vertical().height(14)
        ]
        .spacing(8);
        for screen in ["Day", "Week", "Reports", "Tags", "Rules", "Settings"] {
            navigation = navigation.push(
                button(screen)
                    .on_press(Message::Screen(screen))
                    .width(Length::Fill)
                    .style(if screen == self.screen {
                        button::primary
                    } else {
                        button::text
                    }),
            );
        }
        navigation = navigation
            .push(space::vertical())
            .push(text("Local & private").size(13))
            .push(text("Activity stays on this computer.").size(11));
        let sidebar = container(navigation)
            .padding(18)
            .width(184)
            .height(Length::Fill)
            .style(panel);
        let date = row![
            button("‹").on_press(Message::Shift(-1)),
            text_input("YYYY-MM-DD", &self.date_text)
                .on_input(Message::Date)
                .width(130),
            button("›").on_press(Message::Shift(1)),
            button("Today").on_press(Message::Today)
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        let toolbar = row![
            date,
            space::horizontal(),
            text_input("Search activity · Ctrl/Cmd F", &self.search)
                .on_input(Message::Search)
                .id("activity-search")
                .width(250)
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let control: Element<'_, Message> = if self.runtime.is_none() {
            button("Start recording").on_press(Message::Start).into()
        } else if self.status["state"] == "paused" {
            button("Resume").on_press(Message::Resume).into()
        } else {
            row![
                button("Pause").on_press(Message::Pause(None)),
                button("15 min").on_press(Message::Pause(Some(900_000)))
            ]
            .spacing(6)
            .into()
        };
        let heading = row![
            column![
                text(if self.screen == "Day" {
                    "Day overview"
                } else {
                    self.screen
                })
                .size(25),
                text(self.date.format("%A, %d %B %Y").to_string()).size(12)
            ]
            .spacing(2),
            space::horizontal(),
            text(self.status_label()).size(13),
            control,
            button(text(format!("Theme: {}", self.theme_preference.label())))
                .on_press(Message::Theme),
            button("Quit").on_press(Message::Quit)
        ]
        .spacing(9)
        .align_y(Alignment::Center);
        let content = if self.screen == "Day" {
            self.day_view()
        } else if self.screen == "Tags" {
            self.tags_view()
        } else if self.screen == "Settings" {
            self.settings()
        } else {
            container(
                column![
                    text(format!("{} is coming in a later milestone", self.screen)).size(22),
                    text("This view will be available in a later milestone."),
                    button("Back to day").on_press(Message::Screen("Day"))
                ]
                .spacing(14),
            )
            .padding(24)
            .into()
        };
        let mut main = column![toolbar, heading].spacing(10);
        main = main.push(button("Undo tag action · Ctrl/Cmd Z").on_press_maybe(
            (!self.tag_undo.is_empty()).then_some(Message::Edit(tagging::Edit::Undo)),
        ));
        if let Some(range) = self.away_range {
            main = main.push(
                container(
                    column![
                        text(format!(
                            "You were away {}–{} ({}). What were you doing?",
                            model::time_label(range.start),
                            model::time_label(range.end),
                            model::duration(range.duration())
                        )),
                        row![
                            button("Tag away time").on_press(Message::Edit(tagging::Edit::AwayTag)),
                            button("Ignore").on_press(Message::Edit(tagging::Edit::IgnoreAway)),
                            button("Only ask after 30 min")
                                .on_press(Message::Edit(tagging::Edit::Threshold(30))),
                            button("Disable prompts").on_press(Message::Edit(
                                tagging::Edit::Threshold(i64::MAX / 60_000)
                            ))
                        ]
                        .spacing(8)
                    ]
                    .spacing(8),
                )
                .padding(10)
                .style(panel),
            );
        }
        if self.away_range.is_some() {
            let mut recent = row![].spacing(6);
            let mut ids = Vec::new();
            for range in self.tag_ranges.iter().rev() {
                let Some(id) = range["tag_id"].as_i64() else {
                    continue;
                };
                if !ids.contains(&id)
                    && self
                        .tags
                        .iter()
                        .any(|t| t["id"] == id && t["archived"] != 1)
                {
                    ids.push(id);
                    let name = model::tag_path(&self.tags, id);
                    recent = recent.push(
                        button(text(name.clone()))
                            .on_press(Message::Edit(tagging::Edit::AwayRecent(name))),
                    );
                    if ids.len() == 4 {
                        break;
                    }
                }
            }
            main = main.push(recent);
        }
        if !self.error.is_empty() {
            main = main.push(
                text(&self.error)
                    .size(13)
                    .color(Color::from_rgb8(200, 75, 75)),
            );
        } else if self.status["state"] == "error" {
            let error = self.status["backends"]
                .as_array()
                .and_then(|backends| {
                    backends
                        .iter()
                        .find_map(|backend| backend["error"].as_str())
                })
                .unwrap_or("Tracking needs attention; check Settings.");
            main = main.push(
                button(text(error).size(12))
                    .on_press(Message::Screen("Settings"))
                    .style(button::danger),
            );
        }
        main = main
            .push(content)
            .push(row![
                text(self.status_label()).size(11),
                space::horizontal(),
                text("Draugr Time · Local & private").size(11)
            ])
            .height(Length::Fill);
        // Track the background as a quad so tiny-skia repaints reused buffers after theme changes.
        let base: Element<'_, Message> = container(row![
            sidebar,
            container(main).padding(16).height(Length::Fill)
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.palette().background.into()),
            ..Default::default()
        })
        .into();
        if self.editor.is_some() {
            let dialog = container(scrollable(column![
                self.tag_editor(),
                text(&self.error).color(Color::from_rgb8(200, 75, 75))
            ]))
            .width(650)
            .max_height(520)
            .padding(10)
            .style(panel);
            widget::stack![
                base,
                widget::opaque(container(dialog).center(Length::Fill).style(|_| {
                    container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.35).into()),
                        ..Default::default()
                    }
                }))
            ]
            .into()
        } else {
            base
        }
    }
}

fn native_theme(dark: bool) -> Theme {
    Theme::custom(
        "Draugr Time",
        iced::theme::Palette {
            background: if dark {
                Color::from_rgb8(14, 25, 35)
            } else {
                Color::from_rgb8(244, 246, 249)
            },
            text: if dark {
                Color::from_rgb8(220, 231, 237)
            } else {
                Color::from_rgb8(36, 55, 70)
            },
            primary: Color::from_rgb8(39, 143, 133),
            success: Color::from_rgb8(56, 162, 128),
            danger: Color::from_rgb8(208, 83, 83),
            warning: Color::from_rgb8(217, 165, 56),
        },
    )
}

#[cfg(test)]
mod theme_render_tests {
    use super::*;
    use iced::advanced::{layout, mouse, renderer, widget::Tree, Layout, Renderer as _};

    #[test]
    fn theme_switch_draws_a_damage_tracked_full_window_background() {
        let (state, _) = State::boot();
        let size = iced::Size::new(1280.0, 820.0);
        let viewport = iced::Rectangle::with_size(size);
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 16.0.into());
        for dark in [false, true, false] {
            let theme = native_theme(dark);
            let mut view = state.view(window::Id::unique());
            let mut tree = Tree::new(view.as_widget());
            let layout = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, size),
            );
            renderer.reset(viewport);
            view.as_widget().draw(
                &tree,
                &mut renderer,
                &theme,
                &renderer::Style {
                    text_color: theme.palette().text,
                },
                Layout::new(&layout),
                mouse::Cursor::Unavailable,
                &viewport,
            );
            assert!(
                renderer.layers().iter().flat_map(|layer| &layer.quads).any(
                    |(quad, background)| quad.bounds == viewport
                        && *background == iced::Background::Color(theme.palette().background)
                ),
                "theme background must be a full-window damage-tracked quad; otherwise reused buffers keep the previous background"
            );
        }
    }
}
fn panel(theme: &Theme) -> container::Style {
    let dark = theme.palette().background.r < 0.5;
    container::Style {
        background: Some(
            if dark {
                Color::from_rgb8(24, 37, 49)
            } else {
                Color::WHITE
            }
            .into(),
        ),
        border: iced::Border {
            width: 1.0,
            color: if dark {
                Color::from_rgb8(42, 57, 69)
            } else {
                Color::from_rgb8(220, 228, 234)
            },
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn run() -> iced::Result {
    let args: Vec<_> = std::env::args().collect();
    let autostart_test = args.iter().any(|arg| arg == "--autostart-test");
    let is_test = args.iter().any(|arg| {
        [
            "--smoke-test",
            "--runtime-test",
            "--lifecycle-test",
            "--autostart-test",
        ]
        .contains(&arg.as_str())
    });
    if is_test && std::env::var_os("DRAUGR_TIME_DATA_DIR").is_none() {
        // Test modes record fixture activity; never write it into the user's history.
        let directory =
            std::env::temp_dir().join(format!("draugr-time-test-{}", std::process::id()));
        std::env::set_var("DRAUGR_TIME_DATA_DIR", directory);
    }
    if autostart_test {
        let result = startup::SystemStartup::new().and_then(|mut provider| {
            // Leave the user's real login item as it was before the test.
            let original = provider.is_enabled()?;
            let enabled = startup::set_enabled(&mut provider, true);
            let disabled = startup::set_enabled(&mut provider, false);
            let restored = startup::set_enabled(&mut provider, original);
            if !enabled? || disabled? {
                return Err("system startup state did not follow enable/disable".into());
            }
            if restored? != original {
                return Err("could not restore the previous startup setting".into());
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                TEST_PASSED.store(true, Ordering::Relaxed);
                eprintln!("Iced system startup registration passed");
                return Ok(());
            }
            Err(error) => {
                eprintln!("Iced system startup registration failed: {error}");
                std::process::exit(1);
            }
        }
    }
    let _instance_lock = match single_instance::SingleInstanceLock::acquire() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            eprintln!("Draugr Time is already running.");
            return Ok(());
        }
        Err(error) => {
            eprintln!("Cannot check whether Draugr Time is already running: {error}");
            std::process::exit(1);
        }
    };
    if !is_test && !args.iter().any(|arg| arg == "--benchmark") {
        if let Err(error) =
            startup::SystemStartup::new().and_then(|mut startup| startup.migrate_legacy())
        {
            eprintln!("Startup registration migration failed: {error}");
        }
    }

    let result = iced::daemon(State::boot, State::update, State::view)
        .title(|_: &State, _| "Draugr Time".to_string())
        .subscription(State::subscription)
        .theme(|state: &State, _| state.theme.clone())
        .run();
    if is_test && !TEST_PASSED.load(Ordering::Relaxed) {
        eprintln!("Iced test exited before completing its checks");
        std::process::exit(1);
    }
    result
}

#[cfg(test)]
mod load_tests {
    use super::*;

    #[test]
    fn clicking_an_application_expands_documents_and_clicking_again_collapses() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let _ = state.update(Message::Tab(1));
        let key = state.rows[0].key.clone();
        let _ = state.update(Message::Highlight(key.to_string()));
        assert!(!state.documents.is_empty());
        assert!(state
            .documents
            .windows(2)
            .all(|rows| rows[0].duration >= rows[1].duration));
        let _ = state.update(Message::Highlight(key.to_string()));
        assert!(state.documents.is_empty());
        let _ = state.update(Message::Highlight(key.to_string()));
        let _ = state.update(Message::Select(Some(Range::new(
            state.day.start,
            state.day.start + 1,
        ))));
        assert!(state.documents.is_empty());
        let _ = state.update(Message::Tab(2));
        assert!(state.documents.is_empty());
    }

    #[test]
    fn normal_launch_starts_with_no_generated_activity() {
        let (state, _) = State::boot();
        assert!(
            state.all.iter().all(|track| track.segments.is_empty()),
            "normal launches must not show synthetic demo history"
        );
    }

    #[test]
    fn stale_load_cannot_replace_a_different_day_or_finish_its_load() {
        let (mut state, _) = State::boot();
        state.fixture = false;
        state.invalidate_load();
        let old_generation = state.load_generation;
        let old_tracks = state.all.clone();
        state.change_date(state.date.succ_opt().unwrap());
        state.loading = true;
        let _ = state.update(Message::Loaded(
            old_generation,
            Ok(Snapshot {
                tracks: old_tracks,
                icons: HashMap::new(),
                ..Snapshot::default()
            }),
        ));
        assert!(state.all.iter().all(|track| track.segments.is_empty()));
        assert!(
            state.loading,
            "an old result must not clear the new load's pending flag"
        );
        assert_eq!(state.loaded_day, None);
    }

    #[test]
    fn stale_live_load_cannot_replace_smoke_fixture() {
        let (mut state, _) = State::boot();
        state.fixture = false;
        let old_generation = state.load_generation;
        state.fixture = true;
        state.all = model::fixture_with_apps(state.day, &state.preview_apps);
        state.invalidate_load();
        let preview = state.all.clone();
        let _ = state.update(Message::Loaded(
            old_generation,
            Ok(Snapshot {
                tracks: model::empty_tracks(),
                icons: HashMap::new(),
                ..Snapshot::default()
            }),
        ));
        assert!(state.fixture);
        assert!(
            state.all == preview,
            "a stale live result must not replace the preview"
        );
        assert_eq!(state.loaded_day, None);
    }

    #[test]
    fn current_load_updates_history_and_finishes_loading() {
        let (mut state, _) = State::boot();
        state.fixture = false;
        state.loading = true;
        let _ = state.update(Message::Loaded(
            state.load_generation,
            Ok(Snapshot {
                tracks: model::empty_tracks(),
                icons: HashMap::new(),
                ..Snapshot::default()
            }),
        ));
        assert!(!state.loading);
        assert_eq!(state.loaded_day, Some(state.day));
        assert!(state.all.iter().all(|track| track.segments.is_empty()));
    }
    #[test]
    fn incremental_snapshots_match_full_history_across_gaps_and_search_changes() {
        use draugr_time_core::{Collection, Storage};
        use draugr_time_daemon::Shared;
        use serde_json::json;
        let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
        let (mut state, _) = State::boot();
        state.day = Range::new(0, 100_000);
        state.all = model::empty_tracks();
        for (now, app) in [
            (1000, "alpha"),
            (2000, "alpha"),
            (10_000, "beta"),
            (11_000, "beta"),
            (12_000, "gamma"),
            (13_000, "gamma"),
        ] {
            {
                let mut storage = shared.storage.lock().unwrap();
                storage
                    .heartbeat("window", &json!({"app": app, "title": app}), now, 3000)
                    .unwrap();
                storage
                    .heartbeat("afk", &json!({"status":"active"}), now, 3000)
                    .unwrap();
            }
            let snapshot =
                Runtime::load(shared.clone(), state.day, state.load_cursor.clone()).unwrap();
            let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
            let storage = shared.storage.lock().unwrap();
            let expected = model::tracks(
                &storage.events(None, 0, 100_000).unwrap(),
                &[],
                &[],
                state.day,
            );
            assert_eq!(state.all, expected, "incremental result at {now}");
        }
        let _ = state.update(Message::Search("GAMMA".into()));
        assert_eq!(state.filtered[2].len(), 1);
        let _ = state.update(Message::Search("alpha".into()));
        assert_eq!(state.filtered[2].len(), 1);
        {
            let mut storage = shared.storage.lock().unwrap();
            let tag = storage
                .put(Collection::Tags, None, json!({"name":"Project"}))
                .unwrap();
            storage
                .put(
                    Collection::TagRanges,
                    None,
                    json!({"tag_id":tag["id"],"start_ts":1000,"end_ts":2000}),
                )
                .unwrap();
        }
        let snapshot = Runtime::load(shared.clone(), state.day, state.load_cursor.clone()).unwrap();
        let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
        let storage = shared.storage.lock().unwrap();
        assert_eq!(
            state.all,
            model::tracks(
                &storage.events(None, 0, 100_000).unwrap(),
                &storage.list(Collection::Tags).unwrap(),
                &storage.list(Collection::TagRanges).unwrap(),
                state.day
            )
        );
    }

    #[test]
    fn segment_strings_are_shared_between_application_and_document_tracks() {
        let tracks = model::fixture(Range::new(0, 86_400_000));
        let app = &tracks[2].segments[0];
        let document = &tracks[3].segments[0];
        assert!(Arc::ptr_eq(
            app.app.as_ref().unwrap(),
            document.app.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(&app.detail, &document.label));
        let _ =
            model::application_documents(&tracks[3].segments, &app.key, Range::new(0, 86_400_000));
    }
}

#[cfg(test)]
mod tagging_tests;
