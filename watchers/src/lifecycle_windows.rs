use super::*;
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
    sync::mpsc,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        Power::{RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification},
        RemoteDesktop::*,
    },
    UI::WindowsAndMessaging::*,
};

thread_local! { static CALLBACK: RefCell<Option<LifecycleCallback>> = RefCell::new(None); }
pub struct NativeComputerWatcher {
    window: usize,
    status: String,
}
fn emit(kind: LifecycleKind) {
    CALLBACK.with(|c| {
        if let Some(callback) = &*c.borrow() {
            callback(LifecycleEvent {
                kind,
                ts: timestamp(),
                source: "Win32 lifecycle notification (receipt)".into(),
            });
        }
    });
}
unsafe extern "system" fn procedure(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: invoked by Windows for our registered window; all handles/parameters
    // are forwarded unchanged. Persistence returns before acknowledging suspend.
    unsafe {
        match message {
            WM_POWERBROADCAST => {
                match w as u32 {
                    PBT_APMSUSPEND => emit(LifecycleKind::Sleep),
                    PBT_APMRESUMEAUTOMATIC => emit(LifecycleKind::Wake),
                    _ => {}
                }
                1
            }
            WM_WTSSESSION_CHANGE => {
                match w as u32 {
                    WTS_SESSION_LOCK => emit(LifecycleKind::Lock),
                    WTS_SESSION_UNLOCK => emit(LifecycleKind::Unlock),
                    _ => {}
                }
                0
            }
            WM_QUERYENDSESSION => {
                if l as u32 & ENDSESSION_LOGOFF == 0 {
                    emit(LifecycleKind::ShutdownRequested);
                }
                1
            }
            WM_ENDSESSION => {
                if w == 0 {
                    emit(LifecycleKind::ShutdownCancelled);
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(window);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(window, message, w, l),
        }
    }
}
impl ComputerWatcher for NativeComputerWatcher {
    fn start(callback: LifecycleCallback) -> Result<Self, WatcherError> {
        let (ready, started) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            CALLBACK.with(|c| *c.borrow_mut() = Some(callback));
            // SAFETY: window class/string buffers live through the message loop;
            // Win32 allocations and notification handles are released on this thread.
            unsafe {
                let class: Vec<u16> = "DraugrTimeLifecycle"
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
                let instance = GetModuleHandleW(null());
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(procedure),
                    hInstance: instance,
                    lpszClassName: class.as_ptr(),
                    ..Default::default()
                };
                if RegisterClassW(&wc) == 0 {
                    let _ = ready.send(Err(std::io::Error::last_os_error().to_string()));
                    return;
                }
                // A hidden top-level window receives session-end broadcasts.
                let window = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    null_mut(),
                    null_mut(),
                    instance,
                    null(),
                );
                if window.is_null() {
                    let _ = ready.send(Err(std::io::Error::last_os_error().to_string()));
                    UnregisterClassW(class.as_ptr(), instance);
                    return;
                }
                let power = RegisterSuspendResumeNotification(window, DEVICE_NOTIFY_WINDOW_HANDLE);
                let session = WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) != 0;
                let mut buffer = null_mut();
                let mut bytes = 0;
                let mut initial = false;
                if session
                    && WTSQuerySessionInformationW(
                        WTS_CURRENT_SERVER_HANDLE,
                        WTS_CURRENT_SESSION,
                        WTSSessionInfoEx,
                        &mut buffer,
                        &mut bytes,
                    ) != 0
                {
                    if !buffer.is_null() && bytes as usize >= std::mem::size_of::<WTSINFOEXW>() {
                        let info = &*buffer.cast::<WTSINFOEXW>();
                        if info.Level == 1 {
                            match info.Data.WTSInfoExLevel1.SessionFlags as u32 {
                                WTS_SESSIONSTATE_LOCK => {
                                    emit(LifecycleKind::Lock);
                                    initial = true;
                                }
                                WTS_SESSIONSTATE_UNLOCK => {
                                    emit(LifecycleKind::Unlock);
                                    initial = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    WTSFreeMemory(buffer.cast());
                }
                let status=format!("Win32 sleep/resume {}; session lock {}; initial lock {}; shutdown intent only; off/history unavailable",if power==0{"unavailable"}else{"notifications"},if session{"WTS notifications"}else{"unavailable"},if initial{"queried"}else{"unknown"});
                if ready.send(Ok((window as usize, status))).is_ok() {
                    let mut message = MSG::default();
                    loop {
                        let result = GetMessageW(&mut message, null_mut(), 0, 0);
                        if result <= 0 {
                            if result < 0 {
                                emit(LifecycleKind::Unavailable);
                            }
                            break;
                        }
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                } else {
                    DestroyWindow(window);
                }
                if session {
                    WTSUnRegisterSessionNotification(window);
                }
                if power != 0 {
                    UnregisterSuspendResumeNotification(power);
                }
                UnregisterClassW(class.as_ptr(), instance);
            }
            CALLBACK.with(|c| *c.borrow_mut() = None);
        });
        let (window, status) = started
            .recv()
            .map_err(|e| WatcherError::Platform(e.to_string()))?
            .map_err(WatcherError::Platform)?;
        Ok(Self { window, status })
    }
    fn capabilities(&self) -> String {
        self.status.clone()
    }
}
impl Drop for NativeComputerWatcher {
    fn drop(&mut self) {
        // SAFETY: the notification window belongs to the service thread; PostMessage
        // requests cleanup there without moving any Win32 object between threads.
        unsafe {
            PostMessageW(self.window as HWND, WM_CLOSE, 0, 0);
        }
    }
}
