use dagdatime_core::Storage;
use dagdatime_daemon::{Shared, Tracker};
use dagdatime_watchers::{MockWatcher, WatcherError, WindowInfo};

fn window(app: &str) -> WindowInfo {
    WindowInfo {
        app: app.into(),
        app_name: app.into(),
        title: format!("{app} title"),
        pid: Some(1),
    }
}

#[test]
fn ten_minute_scenario_records_changes_away_and_return() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test-token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let mut watcher = MockWatcher::new(Some(window("editor")), 0);
    for second in 0..=600 {
        watcher.window = Ok(Some(window(if second < 120 {
            "editor"
        } else {
            "browser"
        })));
        watcher.idle = Ok(if (180..540).contains(&second) {
            (second - 180) as u64
        } else {
            0
        });
        tracker
            .poll_once(second * 1_000, &watcher, &watcher)
            .unwrap();
    }
    let storage = shared.storage.lock().unwrap();
    let afk = storage.events(Some("afk"), 0, 601_000).unwrap();
    assert_eq!(afk.len(), 3);
    assert_eq!(afk[0].data["status"], "active");
    assert_eq!((afk[1].start_ts, afk[1].end_ts), (481_000, 540_000));
    assert_eq!(afk[1].data["status"], "away");
    let windows = storage.events(Some("window"), 0, 601_000).unwrap();
    assert_eq!(windows.len(), 3);
    assert_eq!((windows[0].start_ts, windows[0].end_ts), (0, 120_000));
    assert_eq!((windows[1].start_ts, windows[1].end_ts), (120_000, 481_000));
    assert_eq!((windows[2].start_ts, windows[2].end_ts), (540_000, 600_000));
}

#[test]
fn pause_gap_is_not_recorded_and_resume_starts_a_new_interval() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    tracker.poll_once(1_000, &watcher, &watcher).unwrap();
    shared.pause(1_500, None).unwrap();
    tracker.poll_once(2_000, &watcher, &watcher).unwrap();
    shared.resume();
    tracker.poll_once(3_000, &watcher, &watcher).unwrap();
    tracker.poll_once(4_000, &watcher, &watcher).unwrap();
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 5_000)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!((events[0].start_ts, events[0].end_ts), (0, 1_500));
    assert_eq!((events[1].start_ts, events[1].end_ts), (3_000, 4_000));
}

#[test]
fn pause_and_resume_between_ticks_still_breaks_the_interval() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    tracker.poll_once(1_000, &watcher, &watcher).unwrap();
    shared.pause(1_200, None).unwrap();
    shared.resume();
    tracker.poll_once(2_000, &watcher, &watcher).unwrap();
    tracker.poll_once(3_000, &watcher, &watcher).unwrap();
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 4_000)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].end_ts, 1_200);
    assert_eq!(events[1].start_ts, 2_000);
}

#[test]
fn missing_backend_is_visible_and_does_not_record_false_activity() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let mut watcher = MockWatcher::new(None, 0);
    watcher.window = Err(WatcherError::Unsupported("install compositor extension"));
    watcher.idle = Err(WatcherError::Unsupported("idle protocol unavailable"));
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    assert_eq!(shared.status().state, "error");
    assert!(shared
        .status()
        .backends
        .iter()
        .all(|backend| backend.state == "unsupported"));
    assert!(shared
        .storage
        .lock()
        .unwrap()
        .events(None, 0, 1_000)
        .unwrap()
        .is_empty());
}

#[test]
fn sleep_gap_is_not_counted_as_work_and_timed_pause_expires() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 5);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    tracker.poll_once(1_000, &watcher, &watcher).unwrap();
    tracker.poll_once(60_000, &watcher, &watcher).unwrap();
    tracker.poll_once(61_000, &watcher, &watcher).unwrap();
    shared.pause(61_000, Some(1_000)).unwrap();
    tracker.poll_once(63_000, &watcher, &watcher).unwrap();
    assert_eq!(shared.status().state, "tracking");
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 64_000)
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].end_ts, 1_000);
    assert_eq!(events[1].start_ts, 60_000);
}
