use crate::{now_ms, paths, Shared, Tracker};
use dagdatime_core::Storage;
use dagdatime_watchers::{IdleWatcher, NativeWatcher, WatcherError, WindowInfo, WindowWatcher};
use std::{
    error::Error,
    time::{Duration, Instant},
};

struct Unavailable(WatcherError);
impl WindowWatcher for Unavailable {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        Err(self.0.clone())
    }
}
impl IdleWatcher for Unavailable {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        Err(self.0.clone())
    }
}

pub fn serve(idle_threshold: u64) -> Result<(), Box<dyn Error>> {
    let directory = paths::data_dir()?;
    std::fs::create_dir_all(&directory)?;
    let token = paths::token(&directory.join("api-token"))?;
    let storage = Storage::open(directory.join("activity.sqlite"))?;
    let shared = Shared::new(storage, token, idle_threshold);
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, paths::PORT))?;
    listener.set_nonblocking(true)?;
    let server_shared = shared.clone();
    let server = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let shutdown = server_shared.clone();
            let result = axum::serve(listener,crate::api::router(server_shared.clone())).with_graceful_shutdown(async move {
                loop {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => { shutdown.control.lock().unwrap().shutdown = true; break; }
                        _ = tokio::time::sleep(Duration::from_millis(100)) => { if shutdown.control.lock().unwrap().shutdown { break; } }
                    }
                }
            }).await;
            server_shared.control.lock().unwrap().shutdown = true;
            result
        })
    });
    let mut tracker = Tracker::new(shared.clone());
    let mut watcher = NativeWatcher::new();
    let mut last_retry = Instant::now();
    eprintln!("DagdaTime daemon listening on localhost:{}", paths::PORT);
    loop {
        if shared.control.lock().unwrap().shutdown {
            break;
        }
        let start = Instant::now();
        if watcher.is_err() && last_retry.elapsed() >= Duration::from_secs(10) {
            watcher = NativeWatcher::new();
            last_retry = Instant::now();
        }
        let result = match &watcher {
            Ok(watcher) => tracker.poll_once(now_ms(), watcher, watcher),
            Err(error) => {
                let unavailable = Unavailable(error.clone());
                tracker.poll_once(now_ms(), &unavailable, &unavailable)
            }
        };
        if let Err(error) = result {
            let mut control = shared.control.lock().unwrap();
            control.status.state = "error".into();
            control.status.error = Some(error.to_string());
        }
        std::thread::sleep(Duration::from_secs(1).saturating_sub(start.elapsed()));
    }
    server.join().map_err(|_| "API server thread panicked")??;
    Ok(())
}
