use dagdatime_core::Storage;
use dagdatime_daemon::{now_ms, paths, Shared, Tracker};
use dagdatime_watchers::{
    IdleWatcher, MockWatcher, NativeWatcher, WatcherError, WindowInfo, WindowWatcher,
};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Default)]
pub struct Runtime(pub Mutex<Option<Shared>>);
trait Source: WindowWatcher + IdleWatcher + Send {}
impl<T: WindowWatcher + IdleWatcher + Send> Source for T {}
struct Sources {
    watcher: Result<Box<dyn Source>, WatcherError>,
    retry: Instant,
    fixture: bool,
}
impl Sources {
    fn new(fixture: bool) -> Self {
        let watcher = if fixture {
            Ok(Box::new(MockWatcher::new(
                Some(WindowInfo {
                    app: "Runtime fixture".into(),
                    title: "Standalone application".into(),
                    pid: Some(std::process::id()),
                }),
                0,
            )) as Box<dyn Source>)
        } else {
            NativeWatcher::new().map(|watcher| Box::new(watcher) as Box<dyn Source>)
        };
        Self {
            watcher,
            retry: Instant::now(),
            fixture,
        }
    }
    fn sample(&mut self) -> MockWatcher {
        if !self.fixture && self.watcher.is_err() && self.retry.elapsed() > Duration::from_secs(10)
        {
            self.watcher = NativeWatcher::new().map(|watcher| Box::new(watcher) as Box<dyn Source>);
            self.retry = Instant::now();
        }
        match &self.watcher {
            Ok(watcher) => MockWatcher {
                window: watcher.active_window(),
                idle: watcher.idle_seconds(),
            },
            Err(error) => MockWatcher {
                window: Err(error.clone()),
                idle: Err(error.clone()),
            },
        }
    }
}

pub fn start(app: &tauri::AppHandle, fixture: bool) -> Result<(), String> {
    let runtime = app.state::<Runtime>();
    let mut current = runtime.0.lock().unwrap();
    if current
        .as_ref()
        .is_some_and(|shared| !shared.control.lock().unwrap().shutdown)
    {
        return Ok(());
    }
    let directory = paths::data_dir().map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let listener=std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST,paths::PORT)).map_err(|_|"The local activity service port is occupied. Close the other DagdaTime instance and retry.".to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let token = paths::token(&directory.join("api-token")).map_err(|error| error.to_string())?;
    let storage =
        Storage::open(directory.join("activity.sqlite")).map_err(|error| error.to_string())?;
    let shared = Shared::new(storage, token, 300);
    let sources = Arc::new(Mutex::new(Sources::new(fixture)));
    dagdatime_daemon::service::spawn_api(shared.clone(), listener);
    let worker = shared.clone();
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut tracker = Tracker::new(worker.clone());
        loop {
            let start = Instant::now();
            if worker.control.lock().unwrap().shutdown {
                break;
            }
            let now = now_ms();
            let paused = {
                let control = worker.control.lock().unwrap();
                control.paused
                    && !control
                        .status
                        .paused_until
                        .is_some_and(|until| now >= until)
            };
            let sample = if paused {
                MockWatcher {
                    window: Err(WatcherError::Platform(
                        "Waiting for next activity sample".into(),
                    )),
                    idle: Err(WatcherError::Platform(
                        "Waiting for next activity sample".into(),
                    )),
                }
            } else {
                let (sender, receiver) = mpsc::channel();
                let sources = sources.clone();
                if handle
                    .run_on_main_thread(move || {
                        let _ = sender.send(sources.lock().unwrap().sample());
                    })
                    .is_err()
                {
                    break;
                }
                match receiver.recv_timeout(Duration::from_secs(2)) {
                    Ok(sample) => sample,
                    Err(_) => MockWatcher {
                        window: Err(WatcherError::Platform(
                            "Desktop activity query timed out".into(),
                        )),
                        idle: Err(WatcherError::Platform(
                            "Desktop activity query timed out".into(),
                        )),
                    },
                }
            };
            if let Err(error) = tracker.poll_once(now_ms(), &sample, &sample) {
                let mut control = worker.control.lock().unwrap();
                control.status.state = "error".into();
                control.status.error = Some(error.to_string());
            }
            std::thread::sleep(Duration::from_secs(1).saturating_sub(start.elapsed()));
        }
    });
    *current = Some(shared);
    Ok(())
}
