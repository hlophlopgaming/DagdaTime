# DagdaTime development

DagdaTime is a local desktop application. One executable contains the window,
tray, automatic tracking, SQLite and localhost API; no separate service is
needed. M0–M4 are implemented. Tag editing, rules, reports and browser URL
collection are later milestones.

Install stable Rust and Node 24. Linux needs GTK3, WebKitGTK4.1, Ayatana
AppIndicator and librsvg development packages. Windows needs MSVC and WebView2;
macOS needs Xcode command-line tools.

```sh
npm ci --prefix app/frontend
npm --prefix app/frontend run build
cargo build --package dagdatime-app
./target/debug/dagdatime-app
```

Release builds use `cargo build --package dagdatime-app --release`. The resulting
`target/release/dagdatime-app` (`.exe` on Windows) is the application. It uses the
system webview. Data lives in the OS local application-data directory under
DagdaTime. Settings → Data folder selects a different folder for the next launch;
existing history stays in the old folder. `DAGDATIME_DATA_DIR` overrides it for
isolated development/testing. Closing hides to the tray; Quit stops tracking.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm --prefix app/frontend test
npm --prefix app/frontend run test:e2e
python3 -m unittest discover -s tests
```

CI builds and launches the desktop on Windows, macOS and Linux. `--smoke-test`
checks that the actual frontend mounts. `--runtime-test` also records mock
activity to a temporary SQLite database within the desktop process. These modes
exit automatically and never collect real activity.

Manual checks: switch between applications and Unicode window titles, leave
input idle and resume, pause from window/tray, close and reopen from tray, quit,
and choose a data folder then restart. On macOS deny/grant DagdaTime Accessibility
permission and verify errors recover. On Windows check protected applications.
On Wayland install the appropriate compositor integration and verify window
changes and idle recovery; see [wayland.md](wayland.md). CI cannot grant real
desktop permissions or exercise every compositor.

Optional developer diagnostics (not required to run the application):

```sh
cargo run --package dagdatime-daemon --bin tracker -- debug
```

X11 integration tests change the active-window property and must run on an
isolated display:

```sh
env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 xvfb-run -a cargo test --package dagdatime-watchers --package dagdatime-daemon -- --ignored --test-threads=1
```

See [PLAN.md](../PLAN.md) and [decisions.md](decisions.md). Runtime assets are
embedded; all application network traffic is restricted to localhost. No telemetry.
