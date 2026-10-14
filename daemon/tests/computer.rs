use draugr_time_core::Storage;
use draugr_time_daemon::{Shared, Tracker};
use draugr_time_watchers::{LifecycleEvent, LifecycleKind, MockWatcher, WatcherError, WindowInfo};

fn setup() -> (Shared, Tracker, MockWatcher) {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 5);
    let tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(
        Some(WindowInfo {
            app: "editor".into(),
            app_name: "Editor".into(),
            title: "Document".into(),
            pid: Some(1),
        }),
        0,
    );
    (shared, tracker, watcher)
}
fn event(kind: LifecycleKind, ts: i64) -> LifecycleEvent {
    LifecycleEvent {
        kind,
        ts,
        source: "mock OS".into(),
    }
}
#[test]
fn focus_failure_preserves_valid_activity_and_diagnostic_reason() {
    let (shared, mut tracker, mut watcher) = setup();
    watcher.window = Err(WatcherError::PermissionDenied("window title"));
    for ts in [1000, 2000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    let status = serde_json::to_value(shared.status()).unwrap();
    assert_eq!(status["computer"]["machine"], "awake");
    assert_eq!(status["computer"]["activity"], "active");
    assert_eq!(status["computer"]["recording"], "unobserved");
    assert!(status["computer"]["reason"]
        .as_str()
        .unwrap()
        .contains("window"));
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("computer"), 0, 3000)
        .unwrap();
    assert_eq!(events[0].data["activity"], "active");
    watcher.idle = Ok(6);
    tracker.poll_once(3000, &watcher, &watcher).unwrap();
    assert_eq!(
        serde_json::to_value(shared.status()).unwrap()["computer"]["activity"],
        "away"
    );
}
#[test]
fn sleep_wake_and_lock_do_not_extend_work_across_os_intervals() {
    let (shared, mut tracker, watcher) = setup();
    for ts in [1000, 2000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    shared.lifecycle(event(LifecycleKind::Sleep, 2500)).unwrap();
    shared
        .lifecycle(event(LifecycleKind::Wake, 60_000))
        .unwrap();
    for ts in [60_000, 61_000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    shared
        .lifecycle(event(LifecycleKind::Lock, 61_500))
        .unwrap();
    tracker.poll_once(62_000, &watcher, &watcher).unwrap();
    shared
        .lifecycle(event(LifecycleKind::Unlock, 63_000))
        .unwrap();
    for ts in [63_000, 64_000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    let storage = shared.storage.lock().unwrap();
    let windows = storage.events(Some("window"), 0, 70_000).unwrap();
    assert_eq!(
        windows
            .iter()
            .map(|e| (e.start_ts, e.end_ts))
            .collect::<Vec<_>>(),
        vec![(1000, 2500), (60_000, 61_500), (63_000, 64_000)]
    );
    let states = storage.events(Some("computer"), 0, 70_000).unwrap();
    let sleep = states
        .iter()
        .find(|e| e.data["machine"] == "asleep")
        .unwrap();
    assert_eq!((sleep.start_ts, sleep.end_ts), (2500, 60_000));
    assert!(states.iter().any(|e| e.data["locked"] == true));
}
#[test]
fn shutdown_request_does_not_claim_power_off_and_cancellation_resumes() {
    let (shared, mut tracker, watcher) = setup();
    tracker.poll_once(1000, &watcher, &watcher).unwrap();
    shared
        .lifecycle(event(LifecycleKind::ShutdownRequested, 1500))
        .unwrap();
    assert_ne!(
        serde_json::to_value(shared.status()).unwrap()["computer"]["machine"],
        "off"
    );
    shared
        .lifecycle(event(LifecycleKind::ShutdownCancelled, 2000))
        .unwrap();
    tracker.poll_once(2000, &watcher, &watcher).unwrap();
    tracker.poll_once(3000, &watcher, &watcher).unwrap();
    assert_eq!(shared.status().state, "tracking");
}
#[test]
fn restart_distinguishes_orderly_stop_from_unexplained_gap() {
    for orderly in [false, true] {
        let (shared, mut tracker, watcher) = setup();
        for ts in [1000, 2000] {
            tracker.poll_once(ts, &watcher, &watcher).unwrap();
        }
        if orderly {
            shared.stop_recording(2000).unwrap();
        }
        let storage = std::mem::replace(
            &mut *shared.storage.lock().unwrap(),
            Storage::open_in_memory().unwrap(),
        );
        let shared = Shared::new(storage, "test".into(), 5);
        let mut next = Tracker::new(shared.clone());
        next.poll_once(10_000, &watcher, &watcher).unwrap();
        next.poll_once(11_000, &watcher, &watcher).unwrap();
        let events = shared
            .storage
            .lock()
            .unwrap()
            .events(Some("computer"), 0, 12_000)
            .unwrap();
        let gap = events
            .iter()
            .find(|e| e.start_ts == 2000 && e.end_ts == 10_000)
            .unwrap();
        assert_eq!(
            gap.data["recording"],
            if orderly { "stopped" } else { "unknown" }
        );
        assert_eq!(gap.data["machine"], "unknown");
        assert_ne!(gap.data["recording"], "unobserved");
    }
}
#[test]
fn pause_and_lifecycle_are_independent_dimensions() {
    let (shared, mut tracker, watcher) = setup();
    tracker.poll_once(1000, &watcher, &watcher).unwrap();
    shared.pause(2000, None).unwrap();
    shared.lifecycle(event(LifecycleKind::Sleep, 2500)).unwrap();
    shared.lifecycle(event(LifecycleKind::Wake, 5000)).unwrap();
    tracker.poll_once(6000, &watcher, &watcher).unwrap();
    assert_eq!(shared.status().state, "paused");
    let computer = serde_json::to_value(shared.status()).unwrap()["computer"].clone();
    assert_eq!(computer["machine"], "awake");
    assert_eq!(computer["recording"], "paused");
    shared.resume();
    tracker.poll_once(7000, &watcher, &watcher).unwrap();
    tracker.poll_once(8000, &watcher, &watcher).unwrap();
    let storage = shared.storage.lock().unwrap();
    let windows = storage.events(Some("window"), 0, 9000).unwrap();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].end_ts, 2000);
}

#[test]
fn missed_notifications_and_crashes_during_sleep_stay_unknown() {
    for sleep_marker in [false, true] {
        let (shared, mut tracker, watcher) = setup();
        for ts in [1000, 2000] {
            tracker.poll_once(ts, &watcher, &watcher).unwrap();
        }
        if sleep_marker {
            shared.lifecycle(event(LifecycleKind::Sleep, 2500)).unwrap();
            let storage = std::mem::replace(
                &mut *shared.storage.lock().unwrap(),
                Storage::open_in_memory().unwrap(),
            );
            let shared = Shared::new(storage, "test".into(), 5);
            let mut next = Tracker::new(shared.clone());
            next.poll_once(10_000, &watcher, &watcher).unwrap();
            let events = shared
                .storage
                .lock()
                .unwrap()
                .events(Some("computer"), 0, 11_000)
                .unwrap();
            assert!(events.iter().any(|e| e.start_ts == 2500
                && e.end_ts == 10_000
                && e.data["recording"] == "unknown"));
            assert!(!events
                .iter()
                .any(|e| e.data["machine"] == "asleep" && e.end_ts == 10_000));
        } else {
            tracker.poll_once(10_000, &watcher, &watcher).unwrap();
            let events = shared
                .storage
                .lock()
                .unwrap()
                .events(Some("computer"), 0, 11_000)
                .unwrap();
            assert!(events.iter().any(|e| e.start_ts == 2000
                && e.end_ts == 10_000
                && e.data["machine"] == "unknown"));
        }
    }
}
#[test]
fn known_lock_does_not_query_window_titles_or_idle() {
    struct Forbidden;
    impl draugr_time_watchers::WindowWatcher for Forbidden {
        fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
            panic!("locked window queried")
        }
    }
    impl draugr_time_watchers::IdleWatcher for Forbidden {
        fn idle_seconds(&self) -> Result<u64, WatcherError> {
            panic!("locked idle queried")
        }
    }
    let (shared, mut tracker, watcher) = setup();
    tracker.poll_once(1000, &watcher, &watcher).unwrap();
    shared.lifecycle(event(LifecycleKind::Lock, 2000)).unwrap();
    tracker.poll_once(3000, &Forbidden, &Forbidden).unwrap();
    assert_eq!(shared.status().computer.locked, Some(true));
    assert_eq!(shared.status().computer.activity, "unknown");
}

#[test]
fn failed_sleep_marker_keeps_os_state_in_memory_and_never_extends_active_time() {
    let path = std::env::temp_dir().join(format!(
        "draugr-sleep-write-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let shared = Shared::new(Storage::open(&path).unwrap(), "test".into(), 5);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(None, 0);
    for ts in [1000, 2000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_sleep BEFORE INSERT ON events WHEN NEW.bucket='computer' AND json_extract(NEW.data,'$.machine')='asleep' BEGIN SELECT RAISE(ABORT,'simulated disk failure'); END;").unwrap();
    assert!(shared.lifecycle(event(LifecycleKind::Sleep, 2500)).is_err());
    assert_eq!(shared.status().computer.machine, "asleep");
    assert!(tracker.poll_once(3000, &watcher, &watcher).is_err());
    shared
        .lifecycle(event(LifecycleKind::Wake, 60_000))
        .unwrap();
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("computer"), 0, 70_000)
        .unwrap();
    assert!(events
        .iter()
        .filter(|e| e.data["activity"] == "active")
        .all(|e| e.end_ts <= 2500));
    drop(db);
    drop(tracker);
    drop(shared);
    std::fs::remove_file(path).unwrap();
}
