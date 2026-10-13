use super::model::{self, Range, Track};
use crate::icons;
use draugr_time_core::{Collection, Storage};
use draugr_time_daemon::{now_ms, paths, Shared, Tracker};
use draugr_time_watchers::{
    IdleWatcher, MockWatcher, NativeWatcher, WatcherError, WindowInfo, WindowWatcher,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub tracks: Vec<Track>,
    pub icons: HashMap<String, Option<String>>,
}
pub struct Runtime {
    pub shared: Shared,
    #[cfg(not(target_os = "linux"))]
    sampler: Sampler,
    #[cfg(not(target_os = "linux"))]
    sender: mpsc::SyncSender<(i64, MockWatcher)>,
}
impl Runtime {
    pub fn start(fixture: bool) -> Result<Self, String> {
        let directory = paths::data_dir().map_err(|e| e.to_string())?;
        let listener = std::net::TcpListener::bind((
            std::net::Ipv4Addr::LOCALHOST,
            if fixture { 0 } else { paths::PORT },
        ))
        .map_err(|_| {
            "Another tracker is using the local API port. Quit it, then press Start recording."
                .to_string()
        })?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let token = paths::token(&directory.join("api-token")).map_err(|e| e.to_string())?;
        let shared = Shared::new(
            Storage::open(directory.join("activity.sqlite")).map_err(|e| e.to_string())?,
            token,
            300,
        );
        draugr_time_daemon::service::spawn_api(shared.clone(), listener);
        let (sender, _) = spawn_worker(shared.clone(), fixture);
        #[cfg(target_os = "linux")]
        drop(sender);
        Ok(Self {
            shared,
            #[cfg(not(target_os = "linux"))]
            sampler: Sampler::new(fixture),
            #[cfg(not(target_os = "linux"))]
            sender,
        })
    }
    #[cfg(not(target_os = "linux"))]
    pub fn pulse(&mut self) {
        let sample = self.sampler.sample(&self.shared);
        let _ = self.sender.try_send((now_ms(), sample));
    }
    pub fn status(&self) -> Value {
        serde_json::to_value(self.shared.status()).unwrap_or(Value::Null)
    }
    pub fn pause(&self, duration: Option<i64>) -> Result<(), String> {
        self.shared.pause(now_ms(), duration)
    }
    pub fn resume(&self) {
        self.shared.resume();
    }
    pub fn load(shared: Shared, range: Range) -> Result<Snapshot, String> {
        let storage = shared.storage.lock().unwrap();
        let events = storage
            .events(None, range.start, range.end)
            .map_err(|e| e.to_string())?;
        let tags = storage.list(Collection::Tags).map_err(|e| e.to_string())?;
        let ranges = storage
            .list(Collection::TagRanges)
            .map_err(|e| e.to_string())?;
        let tracks = model::tracks(&events, &tags, &ranges, range);
        drop(storage);
        let apps = tracks[2]
            .segments
            .iter()
            .filter_map(|segment| Some((segment.app.clone()?, segment.app_name.clone()?)))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        Ok(Snapshot {
            tracks,
            icons: icons::application_icons(apps),
        })
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.shared.control.lock().unwrap().shutdown = true;
    }
}

struct Sampler {
    source: Result<NativeWatcher, WatcherError>,
    fixture: bool,
    retry: Instant,
}
impl Sampler {
    fn new(fixture: bool) -> Self {
        Self {
            source: if fixture {
                Err(WatcherError::Unsupported("Test fixture"))
            } else {
                NativeWatcher::new()
            },
            fixture,
            retry: Instant::now(),
        }
    }
    fn sample(&mut self, shared: &Shared) -> MockWatcher {
        let now = now_ms();
        let paused = {
            let control = shared.control.lock().unwrap();
            control.paused
                && !control
                    .status
                    .paused_until
                    .is_some_and(|until| until <= now)
        };
        if paused {
            MockWatcher {
                window: Err(WatcherError::Platform("Paused".into())),
                idle: Err(WatcherError::Platform("Paused".into())),
            }
        } else if self.fixture {
            MockWatcher::new(
                Some(WindowInfo {
                    app: std::env::current_exe()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "Runtime fixture".into()),
                    app_name: "Draugr Time".into(),
                    title: "Iced runtime fixture".into(),
                    pid: Some(std::process::id()),
                }),
                0,
            )
        } else {
            if self.source.is_err() && self.retry.elapsed() > Duration::from_secs(10) {
                self.source = NativeWatcher::new();
                self.retry = Instant::now();
            }
            match &self.source {
                Ok(source) => MockWatcher {
                    window: source.active_window(),
                    idle: source.idle_seconds(),
                },
                Err(error) => MockWatcher {
                    window: Err(error.clone()),
                    idle: Err(error.clone()),
                },
            }
        }
    }
}

fn spawn_worker(
    shared: Shared,
    fixture: bool,
) -> (
    mpsc::SyncSender<(i64, MockWatcher)>,
    std::thread::JoinHandle<()>,
) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = std::thread::spawn(move || {
        let mut tracker = Tracker::new(shared.clone());
        #[cfg(target_os = "linux")]
        let mut sampler = Sampler::new(fixture);
        #[cfg(target_os = "linux")]
        let _receiver = receiver;
        #[cfg(not(target_os = "linux"))]
        let _ = fixture;
        while !shared.control.lock().unwrap().shutdown {
            #[cfg(target_os = "linux")]
            let (now, sample) = {
                let sample = sampler.sample(&shared);
                (now_ms(), sample)
            };
            #[cfg(not(target_os = "linux"))]
            let (now, sample) = match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(sample) => sample,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if let Err(error) = tracker.poll_once(now, &sample, &sample) {
                let mut control = shared.control.lock().unwrap();
                control.status.state = "error".into();
                control.status.error = Some(error.to_string());
            }
            #[cfg(target_os = "linux")]
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    (sender, handle)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn recording_continues_without_gui_pulses() {
        let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
        let (sender, worker) = spawn_worker(shared.clone(), true);
        drop(sender);
        std::thread::sleep(Duration::from_millis(2200));
        shared.control.lock().unwrap().shutdown = true;
        worker.join().unwrap();
        let events = shared
            .storage
            .lock()
            .unwrap()
            .events(Some("afk"), 0, i64::MAX)
            .unwrap();
        assert_eq!(
            events.len(),
            1,
            "background recording must not need GUI pulses"
        );
        assert!(events[0].end_ts - events[0].start_ts >= 1000);
        let windows = shared
            .storage
            .lock()
            .unwrap()
            .events(Some("window"), 0, i64::MAX)
            .unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start_ts, events[0].start_ts);
        assert_eq!(windows[0].end_ts, events[0].end_ts);
    }
}
