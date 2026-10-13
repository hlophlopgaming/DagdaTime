use super::{IdleWatcher, WatcherError, WindowInfo, WindowWatcher};
use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    string::{CFString, CFStringRef},
};
use objc2::{rc::autoreleasepool, MainThreadMarker};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSDate, NSRunLoop};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

pub struct MacWatcher;

impl MacWatcher {
    pub fn new() -> Result<Self, WatcherError> {
        MainThreadMarker::new().ok_or_else(|| {
            WatcherError::Platform("macOS watcher must run on the main thread".into())
        })?;
        Ok(Self)
    }
}

fn attribute(element: &CFType, name: &str) -> Result<Option<CFType>, WatcherError> {
    let name = CFString::new(name);
    let mut value = std::ptr::null();
    // SAFETY: caller supplies an AX element; output uses the Copy ownership rule.
    let status = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    };
    match status {
        0 if !value.is_null() => {
            // SAFETY: successful Copy call returned a non-null owned CF value.
            Ok(Some(unsafe { CFType::wrap_under_create_rule(value) }))
        }
        // kAXErrorAttributeUnsupported and kAXErrorNoValue: no window/title.
        -25205 | -25212 => Ok(None),
        -25211 => Err(WatcherError::PermissionDenied("macOS Accessibility: grant access to DagdaTime (or the terminal when using the debug CLI) in System Settings")),
        other => Err(WatcherError::Platform(format!("Accessibility attribute returned {other}"))),
    }
}

impl WindowWatcher for MacWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        MainThreadMarker::new().ok_or_else(|| {
            WatcherError::Platform("macOS window polling must run on the main thread".into())
        })?;
        autoreleasepool(|_| {
            // NSWorkspace caches application state; process its run loop before polling.
            NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.001));
            let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
                return Ok(None);
            };
            // SAFETY: AXIsProcessTrusted has no pointer arguments or ownership effects.
            if unsafe { AXIsProcessTrusted() } == 0 {
                return Err(WatcherError::PermissionDenied("macOS Accessibility: grant access to DagdaTime (or the terminal when using the debug CLI) in System Settings"));
            }
            let pid = app.processIdentifier();
            let app_name = app
                .bundleIdentifier()
                .or_else(|| app.localizedName())
                .map(|name| name.to_string())
                .unwrap_or_default();
            // SAFETY: pid comes from NSRunningApplication; Create returns an owned AX element.
            let application = unsafe { AXUIElementCreateApplication(pid) };
            if application.is_null() {
                return Err(WatcherError::Platform(
                    "cannot create Accessibility application element".into(),
                ));
            }
            // SAFETY: non-null result follows the Create ownership rule.
            let application = unsafe { CFType::wrap_under_create_rule(application) };
            // Bound messaging to unresponsive applications so polling can recover.
            // SAFETY: application is a live AX element and timeout is positive.
            let status = unsafe { AXUIElementSetMessagingTimeout(application.as_CFTypeRef(), 0.5) };
            if status != 0 {
                return Err(WatcherError::Platform(format!(
                    "Accessibility timeout returned {status}"
                )));
            }
            let title = match attribute(&application, "AXFocusedWindow")? {
                None => String::new(),
                Some(window) => {
                    // SAFETY: AXUIElementGetTypeID takes no arguments.
                    if window.type_of() != unsafe { AXUIElementGetTypeID() } {
                        return Err(WatcherError::Platform(
                            "focused window is not an AX element".into(),
                        ));
                    }
                    match attribute(&window, "AXTitle")? {
                        None => String::new(),
                        Some(title) => title
                            .downcast::<CFString>()
                            .ok_or_else(|| {
                                WatcherError::Platform("window title is not a string".into())
                            })?
                            .to_string(),
                    }
                }
            };
            Ok(Some(WindowInfo {
                app: app_name,
                title,
                pid: u32::try_from(pid).ok(),
            }))
        })
    }
}

fn idle_from_seconds(seconds: f64) -> Result<u64, WatcherError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(WatcherError::Platform("invalid macOS idle time".into()));
    }
    Ok(seconds as u64)
}

impl IdleWatcher for MacWatcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        // SAFETY: combined-session state is 0 and any-input event type is u32::MAX.
        idle_from_seconds(unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_time_truncates_subseconds_and_rejects_invalid_values() {
        assert_eq!(idle_from_seconds(12.9).unwrap(), 12);
        for value in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(idle_from_seconds(value).is_err());
        }
    }
}
