#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod embedded;
mod icons;
mod local;
mod shell;

use serde_json::{json, Value};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

struct Smoke {
    enabled: bool,
    runtime: bool,
    ready: std::sync::atomic::AtomicBool,
}

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
async fn start_tracking(app: tauri::AppHandle) -> Result<(), String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(embedded::start(&handle, false));
    })
    .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        receiver.recv().map_err(|error| error.to_string())?
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
fn frontend_ready(app: tauri::AppHandle, smoke: tauri::State<'_, Smoke>) {
    smoke.ready.store(true, std::sync::atomic::Ordering::SeqCst);
    if smoke.enabled && !smoke.runtime {
        app.exit(0);
    }
}

#[tauri::command]
fn data_directory() -> Result<Value, String> {
    use dagdatime_daemon::paths;
    let current = paths::data_dir().map_err(|e| e.to_string())?;
    let choice = paths::read_choice(
        &paths::default_dir()
            .map_err(|e| e.to_string())?
            .join("preferences.json"),
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({"current":current,"pending":choice.filter(|path|*path!=current)}))
}

#[tauri::command]
async fn choose_data_directory(app: tauri::AppHandle) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(folder) = app.dialog().file().blocking_pick_folder() {
            let path = folder.into_path().map_err(|e| e.to_string())?;
            let config = dagdatime_daemon::paths::default_dir()
                .map_err(|e| e.to_string())?
                .join("preferences.json");
            dagdatime_daemon::paths::save_choice(&config, &path).map_err(|e| e.to_string())?;
        }
        data_directory()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn application_icons(apps: Vec<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let icons =
            icons::application_icons(apps.into_iter().map(|app| (app.clone(), app)).collect());
        let icons = icons
            .into_iter()
            .map(|(identity, icon)| {
                let process = identity.split('\0').next().unwrap_or(&identity);
                (process.to_string(), icon)
            })
            .collect::<std::collections::HashMap<_, _>>();
        serde_json::to_value(icons).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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
    let runtime_test = std::env::args().any(|argument| argument == "--runtime-test");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show(app, false)
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(Smoke {
            enabled: smoke || runtime_test,
            runtime: runtime_test,
            ready: std::sync::atomic::AtomicBool::new(false),
        })
        .manage(embedded::Runtime::default())
        .invoke_handler(tauri::generate_handler![
            api_request,
            start_tracking,
            frontend_ready,
            data_directory,
            choose_data_directory,
            application_icons
        ])
        .setup(move |app| {
            if !smoke && !runtime_test {
                if let Err(error) = setup_tray(app) {
                    eprintln!("Tray unavailable: {error}");
                }
                if let Err(error) = embedded::start(app.handle(), false) {
                    eprintln!("Activity service unavailable: {error}");
                }
            } else {
                if runtime_test {
                    embedded::start(app.handle(), true)?;
                }
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    if runtime_test {
                        for _ in 0..100 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let ready = handle
                                .state::<Smoke>()
                                .ready
                                .load(std::sync::atomic::Ordering::SeqCst);
                            let runtime = handle.state::<embedded::Runtime>();
                            let good = runtime.0.lock().unwrap().as_ref().is_some_and(|shared| {
                                shared
                                    .storage
                                    .lock()
                                    .unwrap()
                                    .events(Some("window"), 0, dagdatime_daemon::now_ms() + 1)
                                    .is_ok_and(|events| {
                                        events.iter().any(|event| event.end_ts > event.start_ts)
                                    })
                            });
                            if ready && good {
                                handle.exit(0);
                                return;
                            }
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_secs(15));
                    }
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
