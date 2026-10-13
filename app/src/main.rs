#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod local;
mod shell;

use serde_json::{json, Value};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

struct Smoke(bool);

#[tauri::command]
async fn api_request(path: String, method: String, body: Value) -> Result<Value, String> {
    if !local::allowed(&path, &method) {
        return Err("Unsupported desktop action".into());
    }
    tauri::async_runtime::spawn_blocking(move || local::request(&path, &method, &body))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn start_tracking() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(local::start)
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn frontend_ready(app: tauri::AppHandle, smoke: tauri::State<'_, Smoke>) {
    if smoke.0 {
        app.exit(0);
    }
}

fn show(app: &tauri::AppHandle, settings: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    if settings {
        let _ = app.emit("open-settings", ());
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open DagdaTime", true, None::<&str>)?;
    let status_item = MenuItem::with_id(app, "status", "Starting", false, None::<&str>)?;
    let activity = MenuItem::with_id(app, "activity", "No current activity", false, None::<&str>)?;
    let total = MenuItem::with_id(app, "total", "Today's active time: —", false, None::<&str>)?;
    let quick = MenuItem::with_id(app, "quick", "Tag recent time", false, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause until resumed", true, None::<&str>)?;
    let pause15 = MenuItem::with_id(app, "pause15", "Pause for 15 minutes", true, None::<&str>)?;
    let pause60 = MenuItem::with_id(app, "pause60", "Pause for 1 hour", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume tracking", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit DagdaTime", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &status_item,
            &activity,
            &total,
            &quick,
            &pause,
            &pause15,
            &pause60,
            &resume,
            &settings,
            &quit,
        ],
    )?;
    TrayIconBuilder::with_id("main-tray")
        .icon(shell::icon([130, 145, 160]))
        .menu(&menu)
        .tooltip("DagdaTime · Starting")
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "open" || id == "settings" {
                show(app, id == "settings");
                return;
            }
            let app = app.clone();
            let id = id.to_owned();
            std::thread::spawn(move || {
                let result = match id.as_str() {
                    "pause" => local::request("/pause", "POST", &json!({})),
                    "pause15" => local::request("/pause", "POST", &json!({"duration_ms":900_000})),
                    "pause60" => {
                        local::request("/pause", "POST", &json!({"duration_ms":3_600_000}))
                    }
                    "resume" => local::request("/resume", "POST", &Value::Null),
                    "quit" => {
                        let _ = local::request("/shutdown", "POST", &Value::Null);
                        app.exit(0);
                        return;
                    }
                    _ => return,
                };
                if let Err(error) = result {
                    let _ = app.emit("action-error", error);
                    show(&app, true);
                }
            });
        })
        .build(app)?;
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        let mut tick = 0u64;
        loop {
            let status = local::request("/status", "GET", &Value::Null).ok();
            let view = shell::view(status.as_ref());
            let _ = status_item.set_text(view.label);
            let app_name = status
                .as_ref()
                .and_then(|value| value["current_window"]["app"].as_str())
                .unwrap_or("No current activity");
            let _ = activity.set_text(app_name);
            let paused = status
                .as_ref()
                .is_some_and(|value| value["state"] == "paused");
            let _ = resume.set_enabled(paused);
            let _ = pause.set_enabled(status.is_some() && !paused);
            let _ = pause15.set_enabled(status.is_some() && !paused);
            let _ = pause60.set_enabled(status.is_some() && !paused);
            if let Some(tray) = handle.tray_by_id("main-tray") {
                let _ = tray.set_icon(Some(shell::icon(view.color)));
                let _ = tray.set_tooltip(Some(format!("DagdaTime · {}", view.label)));
            }
            if tick.is_multiple_of(15) {
                use chrono::{Datelike, TimeZone};
                let now = chrono::Local::now();
                if let Some(start) = chrono::Local
                    .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                    .earliest()
                {
                    if let Ok(Value::Array(events)) = local::request(
                        &format!(
                            "/events?bucket=afk&from={}&to={}",
                            start.timestamp_millis(),
                            now.timestamp_millis()
                        ),
                        "GET",
                        &Value::Null,
                    ) {
                        let milliseconds: i64 = events
                            .iter()
                            .filter(|event| event["data"]["status"] == "active")
                            .map(|event| {
                                (event["end_ts"]
                                    .as_i64()
                                    .unwrap_or(0)
                                    .min(now.timestamp_millis())
                                    - event["start_ts"]
                                        .as_i64()
                                        .unwrap_or(0)
                                        .max(start.timestamp_millis()))
                                .max(0)
                            })
                            .sum();
                        let _ = total.set_text(format!(
                            "Today's active time: {}h {}m",
                            milliseconds / 3_600_000,
                            milliseconds / 60_000 % 60
                        ));
                    }
                }
            }
            tick = tick.wrapping_add(1);
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    Ok(())
}

fn main() {
    let smoke = std::env::args().any(|argument| argument == "--smoke-test");
    tauri::Builder::default()
        .manage(Smoke(smoke))
        .invoke_handler(tauri::generate_handler![
            api_request,
            start_tracking,
            frontend_ready
        ])
        .setup(move |app| {
            if !smoke {
                if let Err(error) = setup_tray(app) {
                    eprintln!("Tray unavailable: {error}");
                }
                std::thread::spawn(|| {
                    if let Err(error) = local::start() {
                        eprintln!("Daemon unavailable: {error}");
                    }
                });
            } else {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(15));
                    handle.exit(1);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.app_handle().tray_by_id("main-tray").is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run DagdaTime");
}
