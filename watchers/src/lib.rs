//! Platform watchers for DagdaTime.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WindowInfo {
    /// Executable path when accessible; legacy/platform identity otherwise.
    pub app: String,
    pub title: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WatcherError {
    #[error("permission required: {0}")]
    PermissionDenied(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("platform error: {0}")]
    Platform(String),
}

pub trait WindowWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError>;
}

pub trait IdleWatcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError>;
}

/// Controllable samples and failures, usable without an OS desktop.
pub struct MockWatcher {
    pub window: Result<Option<WindowInfo>, WatcherError>,
    pub idle: Result<u64, WatcherError>,
}

impl MockWatcher {
    pub fn new(window: Option<WindowInfo>, idle_seconds: u64) -> Self {
        Self {
            window: Ok(window),
            idle: Ok(idle_seconds),
        }
    }
}

impl WindowWatcher for MockWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        self.window.clone()
    }
}

impl IdleWatcher for MockWatcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        self.idle.clone()
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use wayland::LinuxWatcher as NativeWatcher;

#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod wayland_idle;
#[cfg(all(test, target_os = "linux"))]
mod wayland_tests;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsWatcher as NativeWatcher;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacWatcher as NativeWatcher;

#[cfg(test)]
mod tests;
