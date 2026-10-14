use super::{IdleWatcher, WatcherError, WindowInfo, WindowWatcher};
use std::{
    cell::RefCell,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, SetLastError},
    System::{
        SystemInformation::GetTickCount,
        Threading::{
            GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        },
    },
};

pub struct WindowsWatcher {
    process: RefCell<Option<Process>>,
    executable: RefCell<Vec<u16>>,
}
struct Process {
    pid: u32,
    handle: OwnedHandle,
    path: String,
    name: String,
}

impl WindowsWatcher {
    pub fn new() -> Result<Self, WatcherError> {
        Ok(Self {
            process: RefCell::new(None),
            executable: RefCell::new(vec![0; 32_768]),
        })
    }
}

fn last_error() -> WatcherError {
    WatcherError::Platform(std::io::Error::last_os_error().to_string())
}

impl WindowWatcher for WindowsWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        // SAFETY: Win32 handles come from the OS; all output buffers are sized
        // for the lengths passed to the APIs and the process handle is owned.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return Ok(None);
            }
            let mut pid = 0;
            if GetWindowThreadProcessId(window, &mut pid) == 0 {
                return Err(last_error());
            }
            SetLastError(0);
            let length = GetWindowTextLengthW(window);
            if length == 0 && GetLastError() != 0 {
                return Err(last_error());
            }
            let mut title = vec![0u16; length as usize + 1];
            SetLastError(0);
            let written = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
            if written == 0 && GetLastError() != 0 {
                return Err(last_error());
            }
            let title = String::from_utf16_lossy(&title[..written as usize]);
            let mut cached = self.process.borrow_mut();
            let valid = cached.as_ref().is_some_and(|process| {
                let mut code = 0;
                process.pid == pid
                    && GetExitCodeProcess(process.handle.as_raw_handle(), &mut code) != 0
                    && code == 259
            });
            if !valid {
                // Keep one process handle: the cached identity cannot become another process
                // when Windows reuses a PID. Never cache a failed lookup.
                *cached = None;
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if handle.is_null() {
                    return Err(last_error());
                }
                let owned = OwnedHandle::from_raw_handle(handle);
                let mut executable = self.executable.borrow_mut();
                let mut size = executable.len() as u32;
                if QueryFullProcessImageNameW(handle, 0, executable.as_mut_ptr(), &mut size) == 0 {
                    return Err(last_error());
                }
                let path = String::from_utf16_lossy(&executable[..size as usize]);
                let name = std::path::Path::new(&path)
                    .file_stem()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                *cached = Some(Process {
                    pid,
                    handle: owned,
                    path,
                    name,
                });
            }
            let process = cached.as_ref().expect("successful process lookup");
            Ok(Some(WindowInfo {
                app: process.path.clone(),
                app_name: process.name.clone(),
                title,
                pid: Some(pid),
            }))
        }
    }
}

fn idle_from_ticks(now: u32, last_input: u32) -> u64 {
    // GetLastInputInfo uses 32-bit ticks, which wrap after about 49.7 days.
    u64::from(now.wrapping_sub(last_input)) / 1_000
}

impl IdleWatcher for WindowsWatcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: initialized, writable structure has the required cbSize.
        unsafe {
            if GetLastInputInfo(&mut info) == 0 {
                return Err(last_error());
            }
            Ok(idle_from_ticks(GetTickCount(), info.dwTime))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_ticks_handle_wraparound_and_truncate_subseconds() {
        assert_eq!(idle_from_ticks(5_999, 1_000), 4);
        assert_eq!(idle_from_ticks(500, u32::MAX - 999), 1);
    }
}
