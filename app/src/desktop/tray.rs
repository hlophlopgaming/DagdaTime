use super::model;
use serde_json::Value;
use std::cell::RefCell;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

pub struct NativeTray {
    icon: TrayIcon,
    status: MenuItem,
    activity: MenuItem,
    total: MenuItem,
    pause: MenuItem,
    resume: MenuItem,
    /// What the tray currently shows. `update` runs every second; menu text and the
    /// icon are only pushed to the OS when they change (on Linux every `set_icon`
    /// writes a new PNG for the indicator).
    shown: RefCell<Option<Shown>>,
}
#[derive(Clone, PartialEq)]
struct Shown {
    label: &'static str,
    color: [u8; 3],
    activity: String,
    total: String,
    pause: bool,
    resume: bool,
}
impl NativeTray {
    pub fn new() -> Result<Self, String> {
        channel();
        #[cfg(target_os = "linux")]
        gtk::init().map_err(|e| e.to_string())?;
        let menu = Menu::new();
        let open = MenuItem::with_id("open", "Open Draugr Time", true, None);
        let start = MenuItem::with_id("start", "Start recording", true, None);
        let status = MenuItem::new("Not recording", false, None);
        let activity = MenuItem::new("No current activity", false, None);
        let total = MenuItem::new("Active time: —", false, None);
        let quick = MenuItem::with_id("quick15", "Tag last 15 minutes", true, None);
        let quick5 = MenuItem::with_id("quick5", "Tag last 5 minutes", true, None);
        let quick60 = MenuItem::with_id("quick60", "Tag last hour", true, None);
        let pause = MenuItem::with_id("pause", "Pause until resumed", false, None);
        let pause15 = MenuItem::with_id("pause15", "Pause for 15 minutes", true, None);
        let resume = MenuItem::with_id("resume", "Resume tracking", false, None);
        let settings = MenuItem::with_id("settings", "Settings", true, None);
        let quit = MenuItem::with_id("quit", "Quit Draugr Time", true, None);
        menu.append_items(&[
            &open, &status, &activity, &total, &start, &quick, &quick5, &quick60, &pause, &pause15,
            &resume, &settings, &quit,
        ])
        .map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_tooltip("Draugr Time · Not recording")
            .with_icon(Self::image([130, 145, 160]))
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            icon,
            status,
            activity,
            total,
            pause,
            resume,
            shown: RefCell::new(None),
        })
    }
    fn image(color: [u8; 3]) -> Icon {
        let mut rgba = vec![0; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                if (x as i32 - 16).pow(2) + (y as i32 - 16).pow(2) < 144 {
                    let index = (y * 32 + x) * 4;
                    rgba[index..index + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
                }
            }
        }
        Icon::from_rgba(rgba, 32, 32).expect("valid tray image")
    }
    pub fn update(&self, status: &Value, active: i64, fixture: bool) {
        let state = status["state"].as_str().unwrap_or("preview");
        let (label, color) = match state {
            "tracking" => ("Tracking", [56, 162, 128]),
            "paused" => ("Paused", [217, 165, 56]),
            "locked" => ("Locked", [160, 137, 181]),
            "asleep" => ("Sleep", [111, 128, 162]),
            "away" => ("AFK", [217, 165, 56]),
            "error" => ("Needs attention", [208, 83, 83]),
            "starting" => ("Starting", [130, 145, 160]),
            _ => ("Not recording", [130, 145, 160]),
        };
        let next = Shown {
            label,
            color,
            activity: status["current_window"]["app"]
                .as_str()
                .map(model::process_label)
                .unwrap_or("No current activity")
                .to_owned(),
            total: format!(
                "{}: {}",
                if fixture {
                    "Fixture active time"
                } else {
                    "Selected day active time"
                },
                model::duration(active)
            ),
            pause: state != "preview" && state != "paused",
            resume: state == "paused",
        };
        let mut shown = self.shown.borrow_mut();
        let old = shown.as_ref();
        if old.is_none_or(|old| old.label != next.label) {
            self.status.set_text(next.label);
        }
        if old.is_none_or(|old| old.activity != next.activity) {
            self.activity.set_text(&next.activity);
        }
        if old.is_none_or(|old| old.total != next.total) {
            self.total.set_text(&next.total);
        }
        if old.is_none_or(|old| old.pause != next.pause) {
            self.pause.set_enabled(next.pause);
        }
        if old.is_none_or(|old| old.resume != next.resume) {
            self.resume.set_enabled(next.resume);
        }
        if old.is_none_or(|old| (old.label, old.color) != (next.label, next.color)) {
            let _ = self.icon.set_icon(Some(Self::image(next.color)));
            let _ = self
                .icon
                .set_tooltip(Some(format!("Draugr Time · {}", next.label)));
        }
        *shown = Some(next);
    }
}

#[cfg(not(target_os = "linux"))]
pub type Tray = NativeTray;
#[derive(Clone, Debug)]
pub enum Event {
    Menu(String),
    Status(Value),
}
fn channel() -> &'static tokio::sync::broadcast::Sender<Event> {
    static CHANNEL: std::sync::OnceLock<tokio::sync::broadcast::Sender<Event>> =
        std::sync::OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (sender, _) = tokio::sync::broadcast::channel(64);
        let events = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = events.send(Event::Menu(event.id.as_ref().to_owned()));
        }));
        sender
    })
}
pub fn push_status(status: Value) {
    let _ = channel().send(Event::Status(status));
}
pub fn events() -> impl iced::futures::Stream<Item = Event> {
    use iced::futures::SinkExt;
    let mut receiver = channel().subscribe();
    iced::stream::channel(64, async move |mut output| loop {
        match receiver.recv().await {
            Ok(event) => {
                if output.send(event).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    })
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    enum Command {
        Update(Value, i64, bool),
        Quit,
    }
    trait Backend {
        fn update(&self, status: &Value, active: i64, fixture: bool);
    }
    impl Backend for NativeTray {
        fn update(&self, status: &Value, active: i64, fixture: bool) {
            NativeTray::update(self, status, active, fixture);
        }
    }
    pub struct Tray {
        sender: mpsc::Sender<Command>,
    }
    impl Tray {
        pub fn new() -> Result<Self, String> {
            spawn_service(NativeTray::new)
        }
        pub fn update(&self, status: &Value, active: i64, fixture: bool) {
            let _ = self
                .sender
                .send(Command::Update(status.clone(), active, fixture));
        }
    }
    impl Drop for Tray {
        fn drop(&mut self) {
            let _ = self.sender.send(Command::Quit);
        }
    }
    fn spawn_service<B: Backend + 'static>(
        init: impl FnOnce() -> Result<B, String> + Send + 'static,
    ) -> Result<Tray, String> {
        let (sender, receiver) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let context = gtk::glib::MainContext::default();
            let _owner = match context.acquire() {
                Ok(owner) => owner,
                Err(error) => {
                    let _ = ready.send(Err(error.to_string()));
                    return;
                }
            };
            let backend = match init() {
                Ok(backend) => backend,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            if ready.send(Ok(())).is_err() {
                return;
            }
            let main_loop = gtk::glib::MainLoop::new(Some(&context), false);
            let stop = main_loop.clone();
            gtk::glib::timeout_add_local(Duration::from_millis(50), move || loop {
                match receiver.try_recv() {
                    Ok(Command::Update(status, active, fixture)) => {
                        backend.update(&status, active, fixture)
                    }
                    Ok(Command::Quit) | Err(mpsc::TryRecvError::Disconnected) => {
                        stop.quit();
                        return gtk::glib::ControlFlow::Break;
                    }
                    Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                }
            });
            main_loop.run();
        });
        started.recv().map_err(|error| error.to_string())??;
        Ok(Tray { sender })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        struct MockBackend(mpsc::Sender<Value>);
        impl Backend for MockBackend {
            fn update(&self, status: &Value, _: i64, _: bool) {
                self.0.send(status.clone()).unwrap();
            }
        }
        #[test]
        fn tray_services_native_events_without_gui_ticks() {
            let (event, received) = mpsc::channel();
            let (updates, updated) = mpsc::channel();
            let tray = spawn_service(move || {
                gtk::glib::idle_add_local_once(move || {
                    event.send(()).unwrap();
                });
                Ok(MockBackend(updates))
            })
            .unwrap();
            let result = received.recv_timeout(Duration::from_secs(1));
            let status = serde_json::json!({"state": "paused"});
            tray.update(&status, 0, false);
            let update = updated.recv_timeout(Duration::from_secs(1));
            drop(tray);
            assert_eq!(update.unwrap(), status);
            assert!(
                result.is_ok(),
                "tray must service native events without GUI ticks"
            );
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::Tray;
