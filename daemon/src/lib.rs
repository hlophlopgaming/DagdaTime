pub mod api;
pub mod paths;
pub mod service;

use draugr_time_core::{Storage, StorageError};
use draugr_time_watchers::{IdleWatcher, WatcherError, WindowInfo, WindowWatcher};
use serde::Serialize;
use serde_json::json;
use std::sync::{Arc, Mutex};

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[derive(Clone, Debug, Serialize)]
pub struct BackendStatus {
    pub name: String,
    pub state: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub state: String,
    pub started_ts: i64,
    pub updated_ts: i64,
    pub paused_until: Option<i64>,
    pub backends: Vec<BackendStatus>,
    pub current_window: Option<WindowInfo>,
    pub idle_seconds: Option<u64>,
    pub error: Option<String>,
}

pub struct Control {
    pub status: Status,
    pub paused: bool,
    pub pause_started: i64,
    pub shutdown: bool,
    pub pause_generation: u64,
}

#[derive(Clone)]
pub struct Shared {
    pub storage: Arc<Mutex<Storage>>,
    pub control: Arc<Mutex<Control>>,
    pub token: Arc<String>,
    pub idle_threshold: u64,
}

impl Shared {
    pub fn new(storage: Storage, token: String, idle_threshold: u64) -> Self {
        let now = now_ms();
        Self {
            storage: Arc::new(Mutex::new(storage)),
            control: Arc::new(Mutex::new(Control {
                status: Status {
                    state: "starting".into(),
                    started_ts: now,
                    updated_ts: now,
                    paused_until: None,
                    backends: vec![],
                    current_window: None,
                    idle_seconds: None,
                    error: None,
                },
                paused: false,
                pause_started: 0,
                shutdown: false,
                pause_generation: 0,
            })),
            token: Arc::new(token),
            idle_threshold,
        }
    }
    pub fn status(&self) -> Status {
        self.control.lock().unwrap().status.clone()
    }
    pub fn pause(&self, now: i64, duration_ms: Option<i64>) -> Result<(), String> {
        if duration_ms.is_some_and(|duration| duration <= 0 || duration > 86_400_000) {
            return Err("pause duration must be between 1 ms and 24 hours".into());
        }
        let mut control = self.control.lock().unwrap();
        if !control.paused {
            control.pause_started = now;
        }
        control.paused = true;
        control.pause_generation = control.pause_generation.wrapping_add(1);
        control.status.state = "paused".into();
        control.status.paused_until = duration_ms.map(|duration| now.saturating_add(duration));
        Ok(())
    }
    pub fn resume(&self) {
        let mut control = self.control.lock().unwrap();
        control.paused = false;
        control.status.paused_until = None;
        control.status.state = "starting".into();
    }
}

pub struct Tracker {
    shared: Shared,
    last_tick: Option<i64>,
    had_window: bool,
    had_idle: bool,
    pause_generation: u64,
}

fn backend(name: &str, error: Option<&WatcherError>) -> BackendStatus {
    BackendStatus {
        name: name.into(),
        state: match error {
            None => "ready",
            Some(WatcherError::Unsupported(_)) => "unsupported",
            Some(WatcherError::PermissionDenied(_)) => "permission_required",
            Some(_) => "error",
        }
        .into(),
        error: error.map(ToString::to_string),
    }
}

impl Tracker {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            last_tick: None,
            had_window: false,
            had_idle: false,
            pause_generation: 0,
        }
    }
    pub fn poll_once(
        &mut self,
        now: i64,
        window: &impl WindowWatcher,
        idle: &impl IdleWatcher,
    ) -> Result<(), StorageError> {
        // Check pause before native queries: paused tracking must not inspect window titles.
        let mut control = self.shared.control.lock().unwrap();
        let expired = control.paused
            && control
                .status
                .paused_until
                .is_some_and(|until| now >= until);
        if expired {
            control.paused = false;
            control.status.paused_until = None;
        }
        if control.paused || expired || control.pause_generation != self.pause_generation {
            if self.last_tick.is_some() && (self.had_window || self.had_idle) {
                let mut storage = self.shared.storage.lock().unwrap();
                let batch = storage.batch()?;
                if self.had_window {
                    batch.extend_last("window", control.pause_started, 3_000)?;
                }
                if self.had_idle {
                    batch.extend_last("afk", control.pause_started, 3_000)?;
                }
                batch.commit()?;
            }
            self.last_tick = None;
            self.had_window = false;
            self.had_idle = false;
            self.pause_generation = control.pause_generation;
            control.status.updated_ts = now;
            control.status.current_window = None;
            if control.paused {
                return Ok(());
            }
        }
        drop(control);
        let window = window.active_window();
        let idle = idle.idle_seconds();
        let mut control = self.shared.control.lock().unwrap();
        // A pause request could arrive while a native query was in flight.
        if control.paused || control.pause_generation != self.pause_generation {
            return Ok(());
        }
        control.status.backends = vec![
            backend("window", window.as_ref().err()),
            backend("idle", idle.as_ref().err()),
        ];
        control.status.current_window = window.as_ref().ok().cloned().flatten();
        control.status.idle_seconds = idle.as_ref().ok().copied();
        control.status.updated_ts = now;
        control.status.error = None;
        // One transaction per tick: one disk sync instead of one per write.
        let mut storage = self.shared.storage.lock().unwrap();
        let batch = storage.batch()?;
        if self
            .last_tick
            .is_some_and(|last| now >= last && now - last < 3_000)
        {
            if self.had_window {
                batch.extend_last("window", now, 3_000)?;
            }
            if self.had_idle {
                batch.extend_last("afk", now, 3_000)?;
            }
        }
        let pulse = if self.last_tick.is_none() { 1 } else { 3_000 };
        self.had_idle = idle.is_ok();
        let away = idle
            .as_ref()
            .is_ok_and(|seconds| *seconds > self.shared.idle_threshold);
        if idle.is_ok() {
            batch.heartbeat(
                "afk",
                &json!({"status": if away { "away" } else { "active" }}),
                now,
                pulse,
            )?;
        }
        self.had_window = false;
        if !away && idle.is_ok() {
            if let Ok(Some(window)) = &window {
                batch.heartbeat("window", &serde_json::to_value(window).unwrap(), now, pulse)?;
                self.had_window = true;
            }
        }
        batch.commit()?;
        drop(storage);
        control.status.state = if window.is_err() || idle.is_err() {
            "error"
        } else if away {
            "away"
        } else {
            "tracking"
        }
        .into();
        self.last_tick = Some(now);
        Ok(())
    }
}
