use draugr_time_core::Storage;
use draugr_time_daemon::{
    preferences::{Preferences, RecordingSchedule},
    Shared, Tracker,
};
use draugr_time_watchers::{MockWatcher, WindowInfo};

#[test]
fn schedule_has_weekday_and_half_open_time_boundaries() {
    let schedule = RecordingSchedule {
        weekdays: vec![1, 2, 3, 4, 5],
        start_minute: 9 * 60,
        end_minute: 19 * 60,
    };
    assert!(!schedule.allows(1, 8 * 60 + 59));
    assert!(schedule.allows(1, 9 * 60));
    assert!(schedule.allows(5, 19 * 60 - 1));
    assert!(!schedule.allows(5, 19 * 60));
    assert!(!schedule.allows(6, 12 * 60));
}
#[test]
fn excluded_apps_never_reach_storage_or_current_activity() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
    shared
        .preferences
        .lock()
        .unwrap()
        .excluded_apps
        .push("secret".into());
    let watcher = MockWatcher::new(
        Some(WindowInfo {
            app: "Secret".into(),
            app_name: "Secret".into(),
            title: "private".into(),
            pid: None,
        }),
        0,
    );
    let mut tracker = Tracker::new(shared.clone());
    for ts in [1000, 2000, 3000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    assert!(shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 4000)
        .unwrap()
        .is_empty());
    assert!(shared.status().current_window.is_none());
    assert_eq!(shared.status().computer.recording, "recording");
}
#[test]
fn invalid_schedule_settings_do_not_replace_saved_preferences() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let mut prefs = Preferences::default();
    prefs.save(&path).unwrap();
    prefs.schedule = Some(RecordingSchedule {
        weekdays: vec![8],
        start_minute: 500,
        end_minute: 400,
    });
    assert!(prefs.save(&path).is_err());
    assert!(Preferences::load(&path).unwrap().schedule.is_none());
}
#[test]
fn scheduled_pause_does_not_record_or_bridge_application_time() {
    use chrono::{Datelike, Timelike};
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
    let watcher = MockWatcher::new(
        Some(WindowInfo {
            app: "editor".into(),
            app_name: "Editor".into(),
            title: "work".into(),
            pid: None,
        }),
        0,
    );
    let mut tracker = Tracker::new(shared.clone());
    let now = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2026, 10, 12, 12, 0, 0)
        .unwrap()
        .timestamp_millis();
    tracker.poll_once(now, &watcher, &watcher).unwrap();
    tracker.poll_once(now + 1000, &watcher, &watcher).unwrap();
    let date = chrono::DateTime::from_timestamp_millis(now + 2000)
        .unwrap()
        .with_timezone(&chrono::Local);
    let weekday = date.weekday().number_from_monday();
    let minute = date.hour() * 60 + date.minute();
    shared.preferences.lock().unwrap().schedule = Some(RecordingSchedule {
        weekdays: vec![if weekday == 7 { 1 } else { weekday + 1 }],
        start_minute: 0,
        end_minute: 1440,
    });
    tracker.poll_once(now + 2000, &watcher, &watcher).unwrap();
    tracker.poll_once(now + 3000, &watcher, &watcher).unwrap();
    assert!(shared.status().current_window.is_none());
    assert_eq!(shared.status().computer.recording, "paused");
    shared.preferences.lock().unwrap().schedule = Some(RecordingSchedule {
        weekdays: vec![weekday],
        start_minute: minute,
        end_minute: 1440,
    });
    tracker.poll_once(now + 4000, &watcher, &watcher).unwrap();
    tracker.poll_once(now + 5000, &watcher, &watcher).unwrap();
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), now, now + 6000)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(events[0].end_ts <= now + 2000);
    assert_eq!(events[1].start_ts, now + 4000);
}
