# Architectural decisions

## 2026-10-13 — M0 workspace

Use four workspace packages: `dagdatime-core`, `dagdatime-watchers`,
`dagdatime-daemon`, and `dagdatime-app`. Prefix package names to avoid a
collision with Rust's `core` crate. Watchers and daemon depend on the core;
the desktop app remains separate from the daemon.

## 2026-10-13 — Empty desktop shell

Use Tauri 2 with an embedded static HTML page for M0. No frontend build tool
is needed for an empty window; choose the UI framework in M4. Enable the
custom protocol by default so `cargo run` and CI release builds use local
embedded assets without a development server. No plugins or remote resources
are enabled. Installer bundling is deferred to M10.

## 2026-10-13 — Validation

Check the skeleton's manifests, local frontend and CI matrix with Python's
standard-library unittest before adding the implementation. CI compiles the
real Tauri application on all three desktop operating systems. Runtime GUI
behavior requires a manual launch; compiling does not verify window rendering.

## 2026-10-13 — M1 SQLite storage and migrations

Use synchronous `rusqlite` with bundled SQLite, avoiding OS-specific SQLite
installations and keeping storage testable in memory on every OS. The initial
numbered SQL migration creates the four tables from PLAN.md and the event
index. Record its version in SQLite `user_version`; apply schema changes and
the version update together in an immediate transaction. Future schema changes
must add a migration, never modify `001_initial.sql`. Reject newer schemas
without changing them. Enable foreign keys on every connection and use a
five-second busy timeout for concurrent access.

## 2026-10-13 — M1 heartbeat semantics

`heartbeat(bucket, data, ts, pulsetime)` accepts JSON values; both time arguments
use milliseconds. The first heartbeat creates `[ts, ts]`. Only the latest event
in the same bucket may merge; compare parsed JSON rather than serialized key
order. Extend its end to `ts` only when data matches and the gap is strictly
less than `pulsetime`, as specified in PLAN.md. Do not extrapolate beyond the
last heartbeat. Require nonnegative Unix timestamps, positive pulsetime and a
nonempty bucket. Reject timestamps earlier than the bucket's last end to avoid
silently shortening or overlapping recorded events; historical import belongs
to M7. Read/merge/insert executes in one immediate transaction so competing
writers cannot race. The core never reads the wall clock or makes network calls.

## 2026-10-13 — Wayland moves to M3

At the user's request, move the GNOME, KDE and wlroots backends and the explicit
unsupported-backend UI acceptance criterion from M8 to M3. Keep M8 reserved
to preserve the remaining milestone identifiers. M2 still covers Windows,
macOS and X11. M3 must provide a visible fallback before the timeline arrives
in M4; it cannot silently ignore unsupported Wayland sessions.

## 2026-10-13 — M2 watcher interfaces and native APIs

Keep `WindowWatcher`, `IdleWatcher`, `WindowInfo` and a controllable `MockWatcher`
in the watchers crate. Errors distinguish permission, unsupported session and
platform failure. Both native and mock implementations use the same traits;
the daemon's debug sampling depends only on those traits.

Use `windows-sys` for foreground window, UTF-16 title, PID, executable name
and current-session idle time. Own process handles with the standard library's
`OwnedHandle`. Compute idle from wrapping 32-bit ticks to match GetLastInputInfo;
idle periods longer than a full 49.7-day counter cycle cannot be reconstructed.
Use `x11rb` EWMH properties and XScreenSaver for X11, with WM_NAME fallback
for legacy titles and WM_CLASS for application identity. Restrict connections
to local Unix-socket displays and reject Wayland sessions, since XWayland cannot
report native Wayland windows. Native Wayland backends arrive in M3.

Use AppKit `NSWorkspace` via `objc2` on the main thread, pumping its run loop
to refresh cached application state. Read the focused window title through
Accessibility; explicitly report missing permission. Keep CoreFoundation Create/
Copy ownership in RAII objects and bound Accessibility messaging to 0.5 seconds.
Use CoreGraphics combined-session idle time. Unsafe FFI stays in OS-specific
watcher modules, never in sampling or storage logic.

API references: [Win32 process image](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew),
[Win32 idle input](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo),
[AppKit frontmost application](https://developer.apple.com/documentation/appkit/nsworkspace/frontmostapplication),
[CoreGraphics idle time](https://developer.apple.com/documentation/coregraphics/cgeventsource/secondssincelasteventtype(_:eventtype:)).

## 2026-10-13 — M2 diagnostic CLI and validation

Name the daemon package's executable `tracker` to provide `tracker debug` as
specified in PLAN.md. Emit one flushed JSON line per sample so window titles
cannot inject extra output lines. Report window and idle errors independently;
retry on the next tick. Poll on a monotonic one-second interval, subtracting
sample time; slow native calls can delay a tick. Do not write events or start
an API server until M3.

Mock tests run without a desktop on every OS. Dedicated Xvfb tests on Linux
verify actual window properties and polling interval, avoiding changes to a
user's desktop. Windows/macOS compilation is checked by CI; real focus, input
and Accessibility permissions still require the manual checks in docs/DEVELOPMENT.md.

## 2026-10-13 — Application UI plan and M3.5 app shell

At the user's request, PLAN.md now specifies the desktop UI explicitly (§5),
modeled on ManicTime's interaction model: a main Day view with timeline tracks,
range selection, a summary panel and tagging, plus Week, Reports, Tags, Rules,
Settings, onboarding, a "while you were away" prompt and a tray menu. A new
milestone M3.5 (app shell: navigation, status, tray, pause/resume, empty and
error states) sits between M3 and M4, so the app is usable as a GUI before the
timeline lands and no milestone numbers change. The UI framework choice moves
from M4 to M3.5. M3 gains `GET /status` and pause/resume endpoints for the UI.

## 2026-10-13 — M3 recording and API

Use Axum on 127.0.0.1:56123, authenticated with a randomly generated local
token (0600 on Unix). Browser Origin requests are denied; the desktop proxies
requests through Tauri commands so the token never reaches frontend JavaScript.
SQLite access is serialized in one shared connection. CRUD uses only a fixed
collection whitelist with bound values, validates ranges and rejects tag cycles.
No schema migration is needed: M1's tables already cover the M3 API.

Poll native watchers on the daemon's main thread (required by macOS AppKit);
the HTTP server runs on a separate Tokio runtime. Close intervals at the next
observed transition, never across a gap of three seconds or more. Do not record
application intervals while away. Pause generations break intervals even when
pause/resume happens between polls; pause also stops native polling. Watcher and
storage failures are visible in status. GUI and daemon remain separate processes.

## 2026-10-13 — M3 Wayland backends

GNOME and KDE use bundled, opt-in compositor bridges on session D-Bus. Reject
stale samples after three seconds. GNOME includes Mutter's idle measurement;
KDE, Sway and Hyprland use ext-idle-notify, preferring version 2 input-only idle.
Sway and Hyprland window data comes from bounded, timed Unix-socket IPC queries.
Unsupported compositors or missing components are errors in `/status`, not empty
success. Idle history before process startup cannot be recovered from the idle
protocol. Installation and compositor checks are in docs/wayland.md.

## 2026-10-13 — M3.5 desktop shell

Choose React 19 + TypeScript with Vite, using React's own state hooks and no
additional UI/state libraries. Compile assets locally and embed them in Tauri;
the frontend has no network client or API token. A small whitelist of Tauri
commands proxies only the necessary localhost routes with redirects/proxies
disabled. Both tray actions and window actions call the same daemon control API.
Closing hides the window when a tray exists; Quit stops the daemon and app.
If the OS cannot provide a tray, closing behaves normally so the app is not
hidden without a way to reopen it. The daemon executable sits beside the app
for development; packaging sidecars/installers remains M10.

Use Node's test runner for pure frontend logic and Playwright for browser UI
tests with mocked Tauri IPC. `--smoke-test` launches the actual desktop WebView
and succeeds only after React mounts and invokes a readiness command; it does
not start recording. Run this launch test in the CI matrix on all three OSes.

## 2026-10-13 — Single desktop executable and selectable data directory

The desktop executable embeds the watcher polling loop, SQLite and localhost
API. It never launches a tracker child process. Native window queries run on
the GUI main thread (required by macOS AppKit); persistence runs in a worker.
A single-instance plugin brings the existing window forward on a second launch.
This supersedes the separate daemon/sidecar runtime decision: the user wants
one desktop application. Closing hides to the tray; Quit stops recording.

Data defaults to the OS local application-data directory. A native folder
picker saves the next-launch directory in the default folder. The current
process keeps its original directory so the token and SQLite cannot diverge.
The UI explicitly states that existing history stays in the old location.

## 2026-10-13 — M4 Day timeline

Use a viewport-sized canvas with subpixel aggregation for 5,000-event days,
React/DOM for controls and summaries. Selection and totals use exact clipped
interval unions rather than rendered pixels. Local calendar boundaries handle
23/25-hour DST days; storage/API remain UTC milliseconds. Unknown gaps are
unobserved, not invented computer-off events. Demo fixtures are opt-in and
never persisted. Tag editing remains M5.

## 2026-10-13 — KDE bridge verified and local application icons

The declarative KWin package requires contents/ui/main.qml. Installation with
kpackagetool6 exposed the incorrect code/ path; correcting it and enabling the
script changed both real KDE/Wayland collectors to ready without restarting
KWin. Add a package-layout regression test. The bridge is installed per-user.

Application and document summaries and timeline tooltips show local system
icons. Linux matches the canonical process executable against desktop-entry
Exec, then uses the app name to disambiguate entries sharing a launcher such as
Steam before resolving the icon through the installed theme. Windows extracts
the executable icon through the Shell/GDI APIs; macOS asks NSWorkspace for the
bundle or executable icon. Lookups return bounded image data and are cached.
Icons are local and require no database schema change. Ambiguous or missing
matches use the stable initial badge.

## 2026-10-13 — Process identity for grouping and icons

At the user's request, newly recorded window `app` values identify the full
executable path when available: Linux resolves /proc/PID/exe, Windows preserves
QueryFullProcessImageNameW, macOS uses NSRunningApplication.executableURL.
The Day summary groups by that path across PID/window-title changes, displays
the basename, and keeps different executables with the same basename separate.
Existing JSON events retain their original identity; there is no table/schema
change or speculative rewrite of historical process identities.

Linux icons require an exact canonical executable match against a desktop
entry's Exec, with Name and StartupWMClass used only to disambiguate entries
that share an executable. ELF executables generally have no embedded icon; the
verified desktop entry supplies it. Missing or ambiguous metadata uses the
fallback badge.

## 2026-10-13 — Iced frontend experiment

Keep the accepted Tauri frontend on `main` while testing Iced on
`experiment/iced`. Build Iced with its tiny-skia software renderer and native
X11/Wayland backends; do not enable wgpu. The standalone `dagdatime-iced`
binary links the existing Rust watcher, tracking and SQLite code directly and
does not launch a helper process. Its first screen is a clearly marked,
non-persistent preview; recording starts only after an explicit user action.
Promotion to `main` depends on the user's desktop and memory comparison.

## 2026-10-13 — Iced login startup and app icon visibility

Add opt-in per-user startup to the Iced experiment using the OS-native XDG
Autostart, macOS LaunchAgent, or Windows Run registration. Launch with
`--autostart --minimized`, which starts collection and keeps the window in the
tray. Use the current executable path; users need to toggle the setting again
after moving the binary. On Linux, populate the preview with up to five
installed applications, using their resolved executable paths and desktop
names so shared launchers resolve correctly without writing preview data. Keep
synthetic placeholders when no installed app icon resolves. Show real process
icons in the Applications summary, and draw them in the Applications track
only when a block is wide enough to read. The same icon provider is used on
Linux, Windows, and macOS.

## 2026-10-13 — One Iced process per user

Use an OS file lock in DagdaTime's default per-user data directory before
starting the Iced event loop. A second launch exits cleanly; the OS releases the
lock when the process exits, including after a crash. Keep the lock independent
of the user-selected history directory so changing storage cannot start a
second copy.

## 2026-10-13 — Iced system, light, and dark themes

Keep a three-state theme choice: System, Light, or Dark. Use Iced's native
system-theme notifications on Windows and macOS. On Linux, read the XDG portal's
appearance color-scheme over the existing session D-Bus connection and poll
while System is selected, because Linux theme detection is disabled with Iced's
default features off. Fall back to Light when the portal has no preference.

## 2026-10-13 — Performance pass: storage I/O, day query, idle UI

Measured with the same SQLite engine (Python sqlite3) on the real schema:

- **Tracking writes.** Each one-second tick ran up to four separate immediate
  transactions in rollback-journal mode with `synchronous=FULL`: ~54 KiB written
  and several fsyncs per second (~4.4 GiB per day of tracking). The database now
  uses WAL with `synchronous=NORMAL` (durable across application crashes; a power
  loss can drop the last commits) and the tracker writes a whole tick through one
  `Storage::batch()` transaction: ~5 KiB per tick, about 10x less, and a tick is
  now atomic. A further step, if needed, is flushing unchanged extensions less
  often than every second.
- **Day query.** `/events` combined `(?1 IS NULL OR bucket=?1)` with only an
  upper bound on `start_ts`, so SQLite scanned the whole history: 60–100 ms per
  request on a year of data (~1M events), growing linearly, while holding the
  storage mutex the tracker needs. Because events within a bucket never overlap
  (heartbeats are nondecreasing; a new event starts at or after the previous end),
  the query seeks to the last event starting at or before `from` and range-scans
  `idx_events_bucket_start` from there; buckets are enumerated with index skips.
  Same rows, 0.1–7 ms regardless of history size, no migration. **Any importer
  (M7) must preserve the non-overlap invariant**, or add an `end_ts` index instead.
- **Desktop shell.** The tray thread read its own process over loopback HTTP every
  second, building a new reqwest blocking client (a thread plus runtime) and
  re-reading the token file each time, and reset the tray icon every second
  (a PNG file write per second on Linux indicators). It now reads the embedded
  service in-process, and only touches menu items and the icon when what they show
  changes. HTTP requests from the UI reuse one client and a cached token, re-read
  once on 401. The tray shows the executable's basename, not its full path.
- **Frontend.** Status (1 s) and day data (10 s) were polled even while the window
  sat hidden in the tray, and past days were re-fetched forever. Polling now stops
  while the window is hidden (Page Visibility plus a `window-visibility` event from
  the shell, since not every WebView reports hiding), refreshes once when shown,
  and only today's data is re-polled. Identical poll results no longer replace
  state, so tracks and the canvas are not rebuilt every 10 s. The Day view is
  memoized against the per-second status refresh, its totals are memoized, and
  application icons for a summary table are fetched in one native call.

## 2026-10-14 — Full comparison of polled datasets

Compare the serialized contents of event, tag and tag-range arrays before
replacing frontend state. IDs and end times alone miss edits to existing tags,
range starts, assignments and notes. An exact comparison also avoids hash
collisions; this runs only on the ten-second history poll.

## 2026-10-14 — Review fixes for the Iced app, icons and release

- **Stale loads.** An Iced history load started before a date change, demo
  toggle or recording start could finish afterwards and replace the new view;
  after toggling the demo on, real history could stay on screen labelled as a
  preview. Loads now carry a generation number and results from an older
  generation are dropped.
- **Idle redraws.** Every 2 s poll rebuilt summaries and cleared the canvas
  cache, even when nothing had changed, and past days were re-read forever.
  Identical snapshots are now ignored, only today is re-polled, and the Linux
  theme portal is polled only while a window is open.
- **Tray.** The Iced tray pushed every menu text and a new icon to the OS once
  per second. It now pushes only what changed, as the Tauri tray does.
- **Linux icon lookup.** Each cache miss re-read every `.desktop` file and
  resolved each Exec through PATH; the preview picker did this per candidate,
  so startup on the UI thread grew quadratically with installed applications.
  Entries are now parsed once into a shared index, rebuilt after 5 minutes so
  new installs are found. Matching rules are unchanged.
- **Test modes.** `--smoke-test`, `--runtime-test` and `--lifecycle-test`
  default to a temporary data directory when `DAGDATIME_DATA_DIR` is unset, so
  a local run cannot write fixture activity into real history.
  `--autostart-test` restores the previous login-item setting.
- **Release archives.** `actions/upload-artifact` drops file modes; the release
  job restores the executable bit before packing the Linux and macOS binaries.
