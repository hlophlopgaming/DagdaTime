use super::{
    linux::X11Watcher, wayland_idle::IdleMonitor, IdleWatcher, WatcherError, WindowInfo,
    WindowWatcher,
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn error(value: impl std::fmt::Display) -> WatcherError {
    WatcherError::Platform(value.to_string())
}

pub enum LinuxWatcher {
    X11(Box<X11Watcher>),
    Wayland(Box<WaylandWatcher>),
}
impl LinuxWatcher {
    pub fn new() -> Result<Self, WatcherError> {
        if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
            || std::env::var_os("WAYLAND_DISPLAY").is_some()
        {
            Ok(Self::Wayland(Box::new(WaylandWatcher::new()?)))
        } else {
            Ok(Self::X11(Box::new(X11Watcher::new()?)))
        }
    }
}
impl WindowWatcher for LinuxWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        let window = match self {
            Self::X11(watcher) => watcher.active_window(),
            Self::Wayland(watcher) => watcher.active_window(),
        }?;
        Ok(window.map(process_identity))
    }
}
pub(crate) fn process_identity(mut window: WindowInfo) -> WindowInfo {
    if window.app_name.is_empty() {
        window.app_name = window.app.clone();
    }
    if let Some(executable) = window
        .pid
        .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/exe")).ok())
    {
        window.app = executable.to_string_lossy().into_owned();
    }
    window
}

impl IdleWatcher for LinuxWatcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        match self {
            Self::X11(watcher) => watcher.idle_seconds(),
            Self::Wayland(watcher) => watcher.idle_seconds(),
        }
    }
}

struct BridgeSample {
    window: Option<WindowInfo>,
    idle_ms: Option<u64>,
    idle_received: Instant,
    received: Instant,
}
#[derive(Clone, Default)]
pub(crate) struct BridgeState(Arc<Mutex<Option<BridgeSample>>>);
impl BridgeState {
    pub(crate) fn update(&self, _: &str, app: &str, title: &str, pid: u32, idle_ms: u64) {
        self.set(app, title, pid, Some(idle_ms));
    }
    fn set(&self, app: &str, title: &str, pid: u32, idle_ms: Option<u64>) {
        let mut guard = self.0.lock().unwrap();
        let idle_received = if idle_ms.is_some() {
            Instant::now()
        } else {
            guard
                .as_ref()
                .map(|s| s.idle_received)
                .unwrap_or_else(Instant::now)
        };
        let idle_ms = idle_ms.or_else(|| guard.as_ref().and_then(|s| s.idle_ms));
        *guard = Some(BridgeSample {
            window: if app.is_empty() && title.is_empty() {
                None
            } else {
                Some(WindowInfo {
                    app: app.into(),
                    app_name: app.into(),
                    title: title.into(),
                    pid: (pid != 0).then_some(pid),
                })
            },
            idle_ms,
            idle_received,
            received: Instant::now(),
        });
    }
    pub(crate) fn heartbeat(&self, idle_ms: Option<u64>) -> bool {
        let mut guard = self.0.lock().unwrap();
        let Some(sample) = guard.as_mut() else {
            return false;
        };
        sample.received = Instant::now();
        if let Some(idle) = idle_ms {
            sample.idle_ms = Some(idle);
            sample.idle_received = sample.received;
        }
        true
    }
    pub(crate) fn window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        let guard = self.0.lock().unwrap();
        let sample = guard.as_ref().filter(|sample| sample.received.elapsed() < Duration::from_secs(3)).ok_or(WatcherError::Unsupported("install/enable the Draugr Time GNOME extension or KWin script; no fresh compositor sample"))?;
        Ok(sample.window.clone())
    }
    pub(crate) fn idle(&self) -> Result<u64, WatcherError> {
        let guard = self.0.lock().unwrap();
        let sample = guard
            .as_ref()
            .filter(|sample| sample.received.elapsed() < Duration::from_secs(3))
            .ok_or(WatcherError::Unsupported(
                "GNOME extension is not sending idle samples",
            ))?;
        sample
            .idle_ms
            .map(|idle| (idle + sample.idle_received.elapsed().as_millis() as u64) / 1_000)
            .ok_or(WatcherError::Unsupported("no compositor idle sample"))
    }
}
struct BridgeService {
    state: BridgeState,
    source: String,
}
#[zbus::interface(name = "org.dagdatime.Watcher")]
impl BridgeService {
    fn update(
        &self,
        source: &str,
        app: &str,
        title: &str,
        pid: &str,
        idle_ms: &str,
    ) -> zbus::fdo::Result<()> {
        self.validate(source, app, title)?;
        let pid = pid
            .parse::<u32>()
            .map_err(|_| zbus::fdo::Error::InvalidArgs("invalid pid".into()))?;
        let idle = idle_ms
            .parse::<u64>()
            .map_err(|_| zbus::fdo::Error::InvalidArgs("invalid idle milliseconds".into()))?;
        if idle > 31_536_000_000 {
            return Err(zbus::fdo::Error::InvalidArgs("idle value too large".into()));
        }
        self.state.update(source, app, title, pid, idle);
        Ok(())
    }
    fn heartbeat(&self, source: &str, idle_ms: &str) -> zbus::fdo::Result<bool> {
        self.validate(source, "", "")?;
        let idle = if idle_ms.is_empty() && source == "kde" {
            None
        } else {
            let value = idle_ms
                .parse::<u64>()
                .map_err(|_| zbus::fdo::Error::InvalidArgs("invalid idle milliseconds".into()))?;
            if value > 31_536_000_000 {
                return Err(zbus::fdo::Error::InvalidArgs("idle value too large".into()));
            }
            Some(value)
        };
        Ok(self.state.heartbeat(idle))
    }
    fn update_window(
        &self,
        source: &str,
        app: &str,
        title: &str,
        pid: &str,
    ) -> zbus::fdo::Result<()> {
        self.validate(source, app, title)?;
        let pid = pid
            .parse::<u32>()
            .map_err(|_| zbus::fdo::Error::InvalidArgs("invalid pid".into()))?;
        self.state.set(app, title, pid, None);
        Ok(())
    }
}
impl BridgeService {
    fn validate(&self, source: &str, app: &str, title: &str) -> zbus::fdo::Result<()> {
        if source != self.source || app.len() > 4_096 || title.len() > 32_768 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "invalid source or oversized window".into(),
            ));
        }
        Ok(())
    }
}

enum WindowBackend {
    Sway(PathBuf),
    Hyprland(PathBuf),
    Bridge(BridgeState, zbus::blocking::Connection),
}
pub struct WaylandWatcher {
    window: WindowBackend,
    idle: Option<Result<IdleMonitor, String>>,
}
impl WaylandWatcher {
    fn new() -> Result<Self, WatcherError> {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();
        let (window, gnome) = if let Some(path) = std::env::var_os("SWAYSOCK") {
            (WindowBackend::Sway(path.into()), false)
        } else if let Ok(signature) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            if signature.contains('/') || signature.contains("..") {
                return Err(error("invalid Hyprland instance signature"));
            }
            let directory = std::env::var_os("XDG_RUNTIME_DIR")
                .ok_or_else(|| error("XDG_RUNTIME_DIR unavailable"))?;
            (
                WindowBackend::Hyprland(
                    PathBuf::from(directory)
                        .join("hypr")
                        .join(signature)
                        .join(".socket.sock"),
                ),
                false,
            )
        } else if desktop.contains("gnome") || desktop.contains("kde") {
            let state = BridgeState::default();
            let source = if desktop.contains("gnome") {
                "gnome"
            } else {
                "kde"
            };
            let connection = zbus::blocking::connection::Builder::session()
                .map_err(error)?
                .name("org.dagdatime.Watcher")
                .map_err(error)?
                .serve_at(
                    "/org/dagdatime/Watcher",
                    BridgeService {
                        state: state.clone(),
                        source: source.into(),
                    },
                )
                .map_err(error)?
                .build()
                .map_err(error)?;
            (WindowBackend::Bridge(state, connection), source == "gnome")
        } else {
            return Err(WatcherError::Unsupported(
                "Wayland compositor unsupported: use GNOME, KDE, Sway or Hyprland",
            ));
        };
        Ok(Self {
            window,
            idle: if gnome {
                None
            } else {
                Some(IdleMonitor::new())
            },
        })
    }
}
fn socket(path: &std::path::Path) -> Result<UnixStream, WatcherError> {
    let stream = UnixStream::connect(path).map_err(error)?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(error)?;
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .map_err(error)?;
    Ok(stream)
}
pub(crate) fn focused_sway(tree: &Value) -> Option<WindowInfo> {
    if tree["focused"].as_bool() == Some(true)
        && (tree["app_id"].is_string() || tree["window_properties"]["class"].is_string())
    {
        return Some(WindowInfo {
            app: tree["app_id"]
                .as_str()
                .or_else(|| tree["window_properties"]["class"].as_str())
                .unwrap_or_default()
                .into(),
            app_name: tree["app_id"]
                .as_str()
                .or_else(|| tree["window_properties"]["class"].as_str())
                .unwrap_or_default()
                .into(),
            title: tree["name"].as_str().unwrap_or_default().into(),
            pid: tree["pid"].as_u64().and_then(|pid| u32::try_from(pid).ok()),
        });
    }
    ["nodes", "floating_nodes"]
        .into_iter()
        .filter_map(|key| tree[key].as_array())
        .flatten()
        .find_map(focused_sway)
}
pub(crate) fn hypr_window(value: &Value) -> Result<Option<WindowInfo>, WatcherError> {
    if value.as_object().is_some_and(|object| object.is_empty()) {
        return Ok(None);
    }
    let app = value["class"]
        .as_str()
        .ok_or_else(|| error("Hyprland class missing"))?;
    if app.is_empty() {
        return Ok(None);
    }
    Ok(Some(WindowInfo {
        app: app.into(),
        app_name: app.into(),
        title: value["title"].as_str().unwrap_or_default().into(),
        pid: value["pid"]
            .as_u64()
            .and_then(|pid| u32::try_from(pid).ok()),
    }))
}
impl WindowWatcher for WaylandWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        match &self.window {
            WindowBackend::Bridge(state, connection) => {
                let _keep_alive = connection;
                state.window()
            }
            WindowBackend::Sway(path) => {
                let mut stream = socket(path)?;
                stream
                    .write_all(b"i3-ipc\0\0\0\0\x04\0\0\0")
                    .map_err(error)?;
                let mut header = [0; 14];
                stream.read_exact(&mut header).map_err(error)?;
                if &header[..6] != b"i3-ipc"
                    || u32::from_le_bytes(header[10..14].try_into().unwrap()) != 4
                {
                    return Err(error("invalid Sway IPC response"));
                }
                let size = u32::from_le_bytes(header[6..10].try_into().unwrap()) as usize;
                if size > 8_388_608 {
                    return Err(error("Sway tree is too large"));
                }
                let mut body = vec![0; size];
                stream.read_exact(&mut body).map_err(error)?;
                Ok(focused_sway(&serde_json::from_slice(&body).map_err(error)?))
            }
            WindowBackend::Hyprland(path) => {
                let mut stream = socket(path)?;
                stream.write_all(b"j/activewindow").map_err(error)?;
                let mut body = Vec::new();
                stream.take(65_537).read_to_end(&mut body).map_err(error)?;
                if body.len() > 65_536 {
                    return Err(error("Hyprland response too large"));
                }
                hypr_window(&serde_json::from_slice(&body).map_err(error)?)
            }
        }
    }
}
impl IdleWatcher for WaylandWatcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        match &self.idle {
            Some(Ok(monitor)) => monitor.seconds(),
            Some(Err(reason)) => Err(error(reason)),
            None => match &self.window {
                WindowBackend::Bridge(state, _) => state.idle(),
                _ => Err(error("idle backend unavailable")),
            },
        }
    }
}
