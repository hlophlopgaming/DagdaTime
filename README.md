# DagdaTime

A local, private desktop time tracker, licensed under GPLv3. The project is
currently at M2: a Rust workspace, an empty Tauri 2 window, tested SQLite
storage and native window/idle watchers. Continuous recording is not implemented yet.
The application makes no external network
calls and has no telemetry.

## Development

Install stable Rust. On Linux, install the Tauri 2 system dependencies
(GTK 3, WebKitGTK 4.1, OpenSSL, Ayatana AppIndicator and librsvg development
packages). Windows requires MSVC build tools and WebView2; macOS requires
Xcode command-line tools.

```sh
python3 -m unittest discover -s tests
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --package dagdatime-app
```

Dependency downloads and CI setup require internet access; application runtime
uses embedded local assets. No Node.js installation is needed for M0.

CI checks Linux, Windows and macOS and builds the desktop executable in release
mode. On each platform, launch the app and verify that one empty window titled
DagdaTime opens and closes normally. On Linux, check both X11 and Wayland
sessions where available. Tracking backends are outside M0.

See [PLAN.md](PLAN.md) for milestones and [docs/decisions.md](docs/decisions.md)
for architectural decisions.

## Watcher debug CLI

```sh
cargo run --package dagdatime-daemon --bin tracker -- debug
```

The `tracker debug` command prints one JSON sample per second, containing
`window` (`app`, `title`, `pid`), `idle_seconds` and `errors`. Stop with Ctrl+C.
It writes no database and sends no telemetry. Missing windows appear as `null`;
watcher errors are reported explicitly, and the other watcher continues working.

Windows uses Win32, Linux uses local X11 EWMH and XScreenSaver, and macOS uses
AppKit/Accessibility and CoreGraphics. Wayland is scheduled for M3; M2 rejects
Wayland sessions rather than treating XWayland as complete desktop coverage.

Manual verification on Windows, macOS and Linux X11:

1. Run the command in a desktop session. Switch between two apps and windows;
   confirm app identity, title (including Unicode) and PID change accordingly.
2. Leave keyboard and mouse untouched for several seconds. Confirm idle time
   increases, then resets after input. Ctrl+C must stop the process.
3. On macOS, first deny Accessibility permission and confirm the explicit error
   alongside working idle readings. Grant the terminal or tracker executable
   access in System Settings → Privacy & Security → Accessibility, then retry.
   Switching apps must keep updating; AppKit is polled on the main thread.
4. On Windows, try a protected/elevated process: failed process queries must
   produce an error and recover on switching to an accessible process.
5. On Wayland, confirm M2 exits with an explicit unsupported-session message.

Linux CI additionally uses Xvfb to exercise a real test window, Unicode and
legacy titles, PID lookup, idle growth, and the CLI's one-second interval:

```sh
env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 xvfb-run -a cargo test --package dagdatime-watchers --package dagdatime-daemon -- --ignored --test-threads=1
```

These tests change the X11 root's active-window property: run them only on a
dedicated display such as Xvfb, never on your working desktop. CI builds the
CLI on all three OSes, but cannot verify real Windows/macOS desktop permissions.

## Storage API

`dagdatime_core::Storage::open(path)` opens or creates a database and applies
its migrations. `Storage::open_in_memory()` provides the same schema for tests.
`heartbeat(bucket, &data, ts, pulsetime)` returns the inserted or extended
`Event`; `data` is a `serde_json::Value`. Timestamps and pulsetime are in
milliseconds, and timestamps must be nondecreasing within a bucket.

An event starts with zero duration. Matching heartbeats extend its end when
the gap is strictly less than pulsetime; a different payload or a gap greater
than or equal to pulsetime creates a new event. Databases with a newer schema
are rejected. No storage is connected to the desktop shell or daemon yet.
