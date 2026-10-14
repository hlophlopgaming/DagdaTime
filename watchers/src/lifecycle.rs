use crate::WatcherError;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleKind {
    Sleep,
    Wake,
    Lock,
    Unlock,
    ShutdownRequested,
    ShutdownCancelled,
    Unavailable,
}
#[derive(Clone, Debug)]
pub struct LifecycleEvent {
    pub kind: LifecycleKind,
    pub ts: i64,
    pub source: String,
}
pub type LifecycleCallback = Arc<dyn Fn(LifecycleEvent) + Send + Sync>;

/// Native lifecycle collection is independent of window/idle availability.
pub trait ComputerWatcher {
    /// Register before taking the initial OS snapshot. The callback must finish
    /// persistence before returning from a pre-sleep notification.
    fn start(callback: LifecycleCallback) -> Result<Self, WatcherError>
    where
        Self: Sized;
    fn capabilities(&self) -> String;
}
pub fn timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
#[cfg(target_os = "linux")]
#[path = "lifecycle_linux.rs"]
mod native;
#[cfg(target_os = "windows")]
#[path = "lifecycle_windows.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "lifecycle_macos.rs"]
mod native;
pub use native::NativeComputerWatcher;
