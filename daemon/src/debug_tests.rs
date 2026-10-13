use super::*;
use dagdatime_watchers::{MockWatcher, WatcherError, WindowInfo};

#[test]
fn debug_output_contains_window_and_idle_in_one_json_line() {
    let watcher = MockWatcher::new(
        Some(WindowInfo {
            app: "editor".into(),
            app_name: "Editor".into(),
            title: "файл\n\"draft\"".into(),
            pid: Some(12),
        }),
        9,
    );
    let mut output = Vec::new();
    debug_tick(&watcher, &watcher, &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert_eq!(text.lines().count(), 1);
    let sample: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(sample["window"]["app"], "editor");
    assert_eq!(sample["window"]["title"], "файл\n\"draft\"");
    assert_eq!(sample["idle_seconds"], 9);
    assert_eq!(sample["errors"], serde_json::json!([]));
}

#[test]
fn window_error_still_reports_idle_and_next_tick_recovers() {
    let mut watcher = MockWatcher::new(None, 30);
    watcher.window = Err(WatcherError::PermissionDenied("Accessibility"));
    let mut output = Vec::new();
    debug_tick(&watcher, &watcher, &mut output).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(sample["window"].is_null());
    assert_eq!(sample["idle_seconds"], 30);
    assert!(sample["errors"][0]
        .as_str()
        .unwrap()
        .contains("Accessibility"));
    watcher.window = Ok(Some(WindowInfo {
        app: "browser".into(),
        app_name: "Browser".into(),
        title: "tab".into(),
        pid: None,
    }));
    output.clear();
    debug_tick(&watcher, &watcher, &mut output).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(sample["window"]["app"], "browser");
    assert_eq!(sample["errors"], serde_json::json!([]));
}

#[test]
fn no_active_window_is_distinct_from_idle_failure() {
    let mut watcher = MockWatcher::new(None, 0);
    watcher.idle = Err(WatcherError::Platform("idle unavailable".into()));
    let mut output = Vec::new();
    debug_tick(&watcher, &watcher, &mut output).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(sample["window"].is_null());
    assert!(sample["idle_seconds"].is_null());
    assert_eq!(sample["errors"].as_array().unwrap().len(), 1);
}

#[test]
fn output_failure_is_returned() {
    struct BrokenOutput;
    impl std::io::Write for BrokenOutput {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let watcher = MockWatcher::new(None, 0);
    assert!(debug_tick(&watcher, &watcher, &mut BrokenOutput).is_err());
}
