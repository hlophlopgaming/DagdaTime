use super::*;
use objc2::{rc::Retained, runtime::ProtocolObject, MainThreadMarker};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillPowerOffNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol};
use std::ptr::NonNull;

pub struct NativeComputerWatcher {
    center: Retained<NSNotificationCenter>,
    observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}
impl ComputerWatcher for NativeComputerWatcher {
    fn start(callback: LifecycleCallback) -> Result<Self, WatcherError> {
        MainThreadMarker::new().ok_or_else(|| {
            WatcherError::Platform("macOS lifecycle registration requires the main thread".into())
        })?;
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let mut observers = Vec::new();
        // SAFETY: AppKit notification names are static. The copied blocks capture
        // only a Send+Sync callback, not borrowed pointers; observers are retained.
        unsafe {
            for (name, kind) in [
                (NSWorkspaceWillSleepNotification, LifecycleKind::Sleep),
                (NSWorkspaceDidWakeNotification, LifecycleKind::Wake),
                (
                    NSWorkspaceWillPowerOffNotification,
                    LifecycleKind::ShutdownRequested,
                ),
            ] {
                let callback = callback.clone();
                let block = block2::RcBlock::new(move |_: NonNull<NSNotification>| {
                    callback(LifecycleEvent {
                        kind,
                        ts: timestamp(),
                        source: "NSWorkspace lifecycle notification (receipt)".into(),
                    })
                });
                observers.push(center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    None,
                    &block,
                ));
            }
        }
        Ok(Self { center, observers })
    }
    fn capabilities(&self) -> String {
        "NSWorkspace sleep/wake and power-off intent; lock and off/history reconstruction unavailable".into()
    }
}
impl Drop for NativeComputerWatcher {
    fn drop(&mut self) {
        // SAFETY: each retained token was returned by this center's registration;
        // remove observers before dropping tokens and their callback captures.
        unsafe {
            for observer in &self.observers {
                self.center.removeObserver((**observer).as_ref());
            }
        }
    }
}
