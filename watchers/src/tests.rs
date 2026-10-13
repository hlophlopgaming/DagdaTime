use super::*;

#[test]
fn mock_implements_both_traits_without_native_apis() {
    let window = WindowInfo {
        app: "editor".into(),
        title: "main.rs".into(),
        pid: Some(42),
    };
    let mut watcher = MockWatcher::new(Some(window.clone()), 7);
    assert_eq!(
        WindowWatcher::active_window(&watcher).unwrap(),
        Some(window)
    );
    assert_eq!(IdleWatcher::idle_seconds(&watcher).unwrap(), 7);
    watcher.window = Ok(None);
    watcher.idle = Ok(12);
    assert_eq!(watcher.active_window().unwrap(), None);
    assert_eq!(watcher.idle_seconds().unwrap(), 12);
}

#[test]
fn mock_can_simulate_permission_errors() {
    let mut watcher = MockWatcher::new(None, 0);
    watcher.window = Err(WatcherError::PermissionDenied("Accessibility"));
    assert!(matches!(
        watcher.active_window(),
        Err(WatcherError::PermissionDenied(_))
    ));
    assert_eq!(watcher.idle_seconds().unwrap(), 0);
}

#[cfg(target_os = "linux")]
#[test]
fn wayland_session_is_explicitly_unsupported_in_m2() {
    assert!(matches!(
        linux::check_session(Some("wayland"), None),
        Err(WatcherError::Unsupported(_))
    ));
    assert!(matches!(
        linux::check_session(None, Some("wayland-0")),
        Err(WatcherError::Unsupported(_))
    ));
    assert!(linux::check_session(Some("x11"), None).is_ok());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a dedicated X11 display (CI uses Xvfb)"]
fn x11_fixture_reads_window_title_pid_and_idle() {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, WindowClass},
        wrapper::ConnectionExt as _,
    };
    let (connection, screen) = x11rb::connect(None).unwrap();
    let root = connection.setup().roots[screen].root;
    let window = connection.generate_id().unwrap();
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            100,
            100,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap()
        .check()
        .unwrap();
    let atom = |name: &[u8]| {
        connection
            .intern_atom(false, name)
            .unwrap()
            .reply()
            .unwrap()
            .atom
    };
    let active = atom(b"_NET_ACTIVE_WINDOW");
    let title = atom(b"_NET_WM_NAME");
    let utf8 = atom(b"UTF8_STRING");
    let pid = atom(b"_NET_WM_PID");
    connection
        .change_property32(
            x11rb::protocol::xproto::PropMode::REPLACE,
            root,
            active,
            AtomEnum::WINDOW,
            &[window],
        )
        .unwrap()
        .check()
        .unwrap();
    connection
        .change_property8(
            x11rb::protocol::xproto::PropMode::REPLACE,
            window,
            title,
            utf8,
            "Документ".as_bytes(),
        )
        .unwrap()
        .check()
        .unwrap();
    connection
        .change_property8(
            x11rb::protocol::xproto::PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"fixture\0FixtureEditor\0",
        )
        .unwrap()
        .check()
        .unwrap();
    connection
        .change_property32(
            x11rb::protocol::xproto::PropMode::REPLACE,
            window,
            pid,
            AtomEnum::CARDINAL,
            &[42],
        )
        .unwrap()
        .check()
        .unwrap();
    let watcher = NativeWatcher::new().unwrap();
    assert_eq!(
        watcher.active_window().unwrap(),
        Some(WindowInfo {
            app: "FixtureEditor".into(),
            title: "Документ".into(),
            pid: Some(42)
        })
    );
    let idle = watcher.idle_seconds().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    assert!(watcher.idle_seconds().unwrap() > idle);
    connection
        .change_property8(
            x11rb::protocol::xproto::PropMode::REPLACE,
            window,
            title,
            utf8,
            b"Changed",
        )
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(watcher.active_window().unwrap().unwrap().title, "Changed");
    connection
        .delete_property(window, title)
        .unwrap()
        .check()
        .unwrap();
    connection
        .change_property8(
            x11rb::protocol::xproto::PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            b"Legacy\xe9",
        )
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(watcher.active_window().unwrap().unwrap().title, "Legacyé");
    connection
        .change_property32(
            x11rb::protocol::xproto::PropMode::REPLACE,
            root,
            active,
            AtomEnum::WINDOW,
            &[0],
        )
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(watcher.active_window().unwrap(), None);
}

#[cfg(target_os = "linux")]
#[test]
fn process_identity_comes_from_pid_not_window_class() {
    let window = super::wayland::process_identity(WindowInfo {
        app: "Misleading window class".into(),
        title: "Any title".into(),
        pid: Some(std::process::id()),
    });
    assert_eq!(
        std::path::PathBuf::from(window.app),
        std::env::current_exe().unwrap()
    );
}
