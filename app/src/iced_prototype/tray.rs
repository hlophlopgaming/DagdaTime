use super::model;
use serde_json::Value;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

pub struct Tray {
    icon: TrayIcon,
    status: MenuItem,
    activity: MenuItem,
    total: MenuItem,
    pause: MenuItem,
    resume: MenuItem,
}
impl Tray {
    pub fn new() -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        gtk::init().map_err(|e| e.to_string())?;
        let menu = Menu::new();
        let open = MenuItem::with_id("open", "Open DagdaTime Iced", true, None);
        let start = MenuItem::with_id("start", "Start recording", true, None);
        let status = MenuItem::new("Preview · no recording", false, None);
        let activity = MenuItem::new("No current activity", false, None);
        let total = MenuItem::new("Active time: —", false, None);
        let quick = MenuItem::new("Tag recent time (M5)", false, None);
        let pause = MenuItem::with_id("pause", "Pause until resumed", false, None);
        let pause15 = MenuItem::with_id("pause15", "Pause for 15 minutes", true, None);
        let resume = MenuItem::with_id("resume", "Resume tracking", false, None);
        let settings = MenuItem::with_id("settings", "Settings", true, None);
        let quit = MenuItem::with_id("quit", "Quit DagdaTime Iced", true, None);
        menu.append_items(&[
            &open, &status, &activity, &total, &start, &quick, &pause, &pause15, &resume,
            &settings, &quit,
        ])
        .map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_tooltip("DagdaTime Iced · Preview")
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
    pub fn update(&self, status: &Value, active: i64, demo: bool) {
        let state = status["state"].as_str().unwrap_or("preview");
        let (label, color) = match state {
            "tracking" => ("Tracking", [56, 162, 128]),
            "paused" => ("Paused", [217, 165, 56]),
            "away" => ("Away", [217, 165, 56]),
            "error" => ("Needs attention", [208, 83, 83]),
            "starting" => ("Starting", [130, 145, 160]),
            _ => ("Preview · no recording", [130, 145, 160]),
        };
        self.status.set_text(label);
        self.pause
            .set_enabled(state != "preview" && state != "paused");
        self.resume.set_enabled(state == "paused");
        let app = status["current_window"]["app"]
            .as_str()
            .map(model::process_label)
            .unwrap_or("No current activity");
        self.activity.set_text(app);
        self.total.set_text(format!(
            "{}: {}",
            if demo {
                "Preview active time"
            } else {
                "Selected day active time"
            },
            model::duration(active)
        ));
        let _ = self.icon.set_icon(Some(Self::image(color)));
        let _ = self
            .icon
            .set_tooltip(Some(format!("DagdaTime Iced · {label}")));
    }
    pub fn events() -> Vec<String> {
        #[cfg(target_os = "linux")]
        {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
        }
        MenuEvent::receiver()
            .try_iter()
            .map(|event| event.id.as_ref().to_owned())
            .collect()
    }
}
