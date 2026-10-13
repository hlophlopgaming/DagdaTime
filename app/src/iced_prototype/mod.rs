//! Native Iced experiment: M4 functionality and resource-use comparison.
mod appearance;
mod model;
mod runtime;
mod single_instance;
mod startup;
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
    Tick,
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
    Zoom(f64),
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
    Demo,
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
    date: NaiveDate,
    date_text: String,
    day: Range,
    view: Range,
    all: Vec<Track>,
    filtered: Vec<Track>,
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
    limit: usize,
    metrics: [i64; 4],
    demo: bool,
    tracks_menu: bool,
    theme: Theme,
    theme_preference: appearance::Preference,
    system_theme: iced::theme::Mode,
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
        let preview_apps = crate::icons::preview_application_processes();
        let icons = decode_icons(crate::icons::application_icons(preview_apps.clone()));
        let tray = if smoke || (runtime_test && !lifecycle) {
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
            date,
            date_text: date.to_string(),
            day,
            view: day,
            all: model::fixture_with_apps(day, &preview_apps),
            filtered: Vec::new(),
            order: vec![0, 1, 2, 3],
            visible: vec![0, 1, 2, 3],
            shown: [true; 4],
            selection: None,
            hover: None,
            highlight: None,
            search: String::new(),
            screen: "Day",
            tab: 1,
            rows: Vec::new(),
            limit: 50,
            metrics: [0; 4],
            demo: true,
            tracks_menu: false,
            theme: native_theme(false),
            theme_preference: appearance::Preference::System,
            system_theme: iced::theme::Mode::None,
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
            self.demo = false;
            self.all = model::empty_tracks();
            self.invalidate_load();
            self.rebuild();
            return;
        }
        match Runtime::start(self.runtime_test) {
            Ok(runtime) => {
                self.runtime = Some(runtime);
                self.demo = false;
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
        self.last_load = Instant::now() - Duration::from_secs(5);
    }
    fn rebuild(&mut self) {
        let ranges = |track: usize, key: Option<&str>| {
            self.all[track]
                .segments
                .iter()
                .filter(|s| key.is_none_or(|key| s.key == key))
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
                    .filter(|s| s.key != "untagged")
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                self.day,
            ),
            model::union_duration(&ranges(0, Some("untagged")), self.day),
        ];
        self.filtered = self.all.clone();
        if !self.search.trim().is_empty() {
            let search = self.search.to_lowercase();
            for track in &mut self.filtered {
                track.segments.retain(|s| {
                    format!("{} {} {}", s.label, s.detail, s.key)
                        .to_lowercase()
                        .contains(&search)
                });
            }
        }
        self.visible = self
            .order
            .iter()
            .copied()
            .filter(|index| self.shown[*index])
            .collect();
        self.summary();
        self.hover = None;
        self.cache.clear();
    }
    fn summary(&mut self) {
        let track = [0, 2, 3][self.tab];
        self.rows = model::summarize(
            &self.filtered[track].segments,
            self.selection.unwrap_or(self.day),
        );
    }
    fn change_date(&mut self, date: NaiveDate) {
        self.date = date;
        self.date_text = date.to_string();
        self.day = model::day_bounds(date);
        self.view = self.day;
        self.selection = None;
        self.highlight = None;
        self.limit = 50;
        if self.demo {
            self.all = model::fixture_with_apps(self.day, &self.preview_apps);
        } else {
            self.all = model::empty_tracks();
            self.invalidate_load();
        }
        self.rebuild();
    }
    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick),
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
            Message::Tick => {
                for event in tray::Tray::events() {
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
                        "quit" => Message::Quit,
                        _ => continue,
                    };
                    let task = self.update(message);
                    if event == "quit" {
                        return task;
                    }
                }
                if self.last_sample.elapsed() >= Duration::from_secs(1) {
                    self.last_sample = Instant::now();
                    if let Some(runtime) = self.runtime.as_mut() {
                        runtime.pulse();
                        self.status = runtime.status();
                    }
                    if let Some(tray) = self.tray.as_ref() {
                        tray.update(&self.status, self.metrics[0], self.demo);
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
                let now = dagdatime_daemon::now_ms();
                let today = self.day.start <= now && now < self.day.end;
                // Only today's history still changes; a past day is loaded once.
                let stale = self.loaded_day != Some(self.day)
                    || (today && self.last_load.elapsed() > Duration::from_secs(2));
                if !self.demo && !self.loading && stale && (self.window.is_some() || self.lifecycle)
                {
                    if let Some(runtime) = self.runtime.as_ref() {
                        let shared = runtime.shared.clone();
                        let day = self.day;
                        let generation = self.load_generation;
                        self.loading = true;
                        self.last_load = Instant::now();
                        return Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || Runtime::load(shared, day))
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
                    // Started before a date, demo or recording change; not for this view.
                    return Task::none();
                }
                self.loading = false;
                match result {
                    Ok(snapshot) => {
                        self.loaded_day = Some(self.day);
                        let new_icons: HashMap<_, _> = snapshot
                            .icons
                            .into_iter()
                            .filter(|(key, _)| !self.icons.contains_key(key))
                            .collect();
                        self.icons.extend(decode_icons(new_icons));
                        self.error.clear();
                        // Unchanged polls must not rebuild summaries or repaint the canvas.
                        if snapshot.tracks != self.all {
                            self.all = snapshot.tracks;
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
                self.cache.clear();
            }
            Message::Zoom(factor) => {
                let anchor = self.selection.map(|s| s.start + s.duration() / 2);
                self.view = model::zoom(self.day, self.view, factor, anchor);
                self.hover = None;
                self.cache.clear();
            }
            Message::Fit => {
                self.view = self.day;
                self.cache.clear();
            }
            Message::Pan(value) => {
                let span = self.view.duration();
                let start = self.day.start
                    + (value.clamp(0.0, 1.0) * (self.day.duration() - span) as f64).round() as i64;
                self.view = Range::new(start, start + span);
                self.cache.clear();
                self.hover = None;
            }
            Message::PanBy(value) => {
                let span = self.view.duration();
                let start = (self.view.start + (value * span as f64) as i64)
                    .clamp(self.day.start, self.day.end - span);
                self.view = Range::new(start, start + span);
                self.cache.clear();
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
            Message::Demo => {
                self.demo = !self.demo;
                if self.demo {
                    self.invalidate_load();
                    self.all = model::fixture_with_apps(self.day, &self.preview_apps);
                } else if self.runtime.is_some() {
                    self.all = model::empty_tracks();
                    self.invalidate_load();
                } else {
                    self.demo = true;
                    self.error = "Start recording to load your local history.".into();
                }
                self.selection = None;
                self.rebuild();
            }
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
                    let result = dagdatime_daemon::paths::default_dir().and_then(|dir| {
                        dagdatime_daemon::paths::save_choice(&dir.join("preferences.json"), &folder)
                    });
                    match result {
                        Ok(()) => self.pending_dir = Some(folder.to_string_lossy().into_owned()),
                        Err(error) => self.error = error.to_string(),
                    }
                }
            }
            Message::Key(key, modifiers) => {
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
                        "+" | "=" => Some(Message::Zoom(0.5)),
                        "-" => Some(Message::Zoom(2.0)),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(message) = message {
                    return self.update(message);
                }
            }
            Message::Screenshot(screenshot) => {
                if let Some(path) = std::env::var_os("DAGDATIME_ICED_SCREENSHOT") {
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
            Some("away") => "● Away",
            Some("error") => "● Needs attention",
            Some("starting") => "● Starting",
            _ => "○ Preview",
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
        let mode = if self.demo {
            "Preview · 5,000 generated events · sample data is never saved"
        } else {
            "Live data · local SQLite · process identity"
        };
        let heading = row![
            text("Activity timeline").size(15),
            space::horizontal(),
            button("Tracks").on_press(Message::Tracks),
            button("−").on_press(Message::Zoom(2.0)),
            text(if self.view == self.day {
                "Full day".into()
            } else {
                model::duration(self.view.duration())
            })
            .size(12),
            button("+").on_press(Message::Zoom(0.5)),
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
            tracks: &self.filtered,
            visible: &self.visible,
            view: self.view,
            selection: self.selection,
            highlight: self.highlight.as_deref(),
            cache: &self.cache,
            stats: &self.stats,
            icons: &self.icons,
        });
        let chart: Element<'_, Message> = if let Some(segment) = self
            .hover
            .and_then(|(track, index)| self.filtered.get(track)?.segments.get(index))
        {
            let details = column![
                text(&segment.label).size(14),
                text(&segment.detail).size(12),
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
        timeline_body=timeline_body.push(row![container(labels).width(124),chart].spacing(8)).push(row![text("Pan across day").size(10).width(124),slider(0.0..=1.0,pan,Message::Pan).step(0.001)].spacing(8)).push(text("Active  ·  Away  ·  Unobserved gaps   |   Drag to select · Shift-click to extend · Double-click a block · Wheel to zoom").size(10));
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
            button("Tag selection (M5)")
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
            let label = row![icon, text(&entry.label).size(13)]
                .spacing(8)
                .align_y(Alignment::Center);
            let style = if self.highlight.as_ref() == Some(&entry.key) {
                button::primary
            } else {
                button::text
            };
            body = body.push(
                row![
                    button(label)
                        .on_press(Message::Highlight(entry.key.clone()))
                        .style(style)
                        .width(Length::Fill),
                    text(model::duration(entry.duration)).size(13).width(90),
                    text(format!("{:.1}%", entry.share)).size(13).width(78)
                ]
                .align_y(Alignment::Center),
            );
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
            backend=backend.push(text("Preview mode: no activity is being recorded. Press Start recording to test the embedded collector."));
        }
        let directory = dagdatime_daemon::paths::data_dir()
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
                        "The folder change takes effect after restarting DagdaTime.".into()
                    })
            )
            .size(12)
        ]
        .spacing(12);
        let autostart = column![
            checkbox(self.autostart_enabled)
                .label("Start DagdaTime when I log in")
                .on_toggle(Message::AutoStart),
            text("Starts activity tracking in the tray. Turn this off here to remove it from system startup.")
                .size(12),
            text(&self.autostart_error)
                .size(12)
                .color(Color::from_rgb8(200, 75, 75))
        ]
        .spacing(8);
        let resource=column![text("Iced experiment").size(22),text("Rust UI · tiny-skia software renderer · no WebKit").size(14),text(format!("Canvas draws: {} · max geometry build: {:.2} ms",self.stats.frames.load(Ordering::Relaxed),self.stats.max_build_us.load(Ordering::Relaxed) as f64/1000.0)).size(13),text("Closing the window releases its renderer and keeps recording. Open it again from the tray. Quit stops the process. Switching themes, tray behavior and OS permissions need desktop verification before promoting this branch.").size(13)].spacing(12);
        scrollable(
            column![
                container(backend).padding(16).style(panel),
                container(folder).padding(16).style(panel),
                container(autostart).padding(16).style(panel),
                container(resource).padding(16).style(panel)
            ]
            .spacing(14),
        )
        .into()
    }
    fn view(&self, _id: window::Id) -> Element<'_, Message> {
        let mut navigation = column![
            text("◷ DagdaTime").size(23),
            text("ICED EXPERIMENT").size(10),
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
            button(if self.demo { "Live data" } else { "View demo" }).on_press(Message::Demo),
            button(text(format!("Theme: {}", self.theme_preference.label())))
                .on_press(Message::Theme),
            button("Quit").on_press(Message::Quit)
        ]
        .spacing(9)
        .align_y(Alignment::Center);
        let content = if self.screen == "Day" {
            self.day_view()
        } else if self.screen == "Settings" {
            self.settings()
        } else {
            container(column![text(format!("{} is coming in a later milestone",self.screen)).size(22),text("This branch compares the Iced Day view with the existing Tauri implementation."),button("Back to day").on_press(Message::Screen("Day"))].spacing(14)).padding(24).into()
        };
        let mut main = column![toolbar, heading].spacing(10);
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
                text("DagdaTime · native Iced prototype · tiny-skia").size(11)
            ])
            .height(Length::Fill);
        row![sidebar, container(main).padding(16).height(Length::Fill)]
            .height(Length::Fill)
            .into()
    }
}

fn native_theme(dark: bool) -> Theme {
    Theme::custom(
        "DagdaTime",
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
    if is_test && std::env::var_os("DAGDATIME_DATA_DIR").is_none() {
        // Test modes record fixture activity; never write it into the user's history.
        let directory = std::env::temp_dir().join(format!("dagdatime-test-{}", std::process::id()));
        std::env::set_var("DAGDATIME_DATA_DIR", directory);
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
            eprintln!("DagdaTime is already running.");
            return Ok(());
        }
        Err(error) => {
            eprintln!("Cannot check whether DagdaTime is already running: {error}");
            std::process::exit(1);
        }
    };
    #[cfg(target_os = "linux")]
    if let Err(error) = gtk::init() {
        eprintln!("GTK tray initialization: {error}");
    }

    let result = iced::daemon(State::boot, State::update, State::view)
        .title(|_: &State, _| "DagdaTime · Iced prototype".to_string())
        .subscription(State::subscription)
        .theme(|state: &State, _| state.theme.clone())
        .run();
    if is_test && !TEST_PASSED.load(Ordering::Relaxed) {
        eprintln!("Iced test exited before completing its checks");
        std::process::exit(1);
    }
    result
}
