# Draugr Time development

One native Iced executable contains the window, tray, watchers, SQLite storage
and localhost API. It uses the tiny-skia software renderer.

Install stable Rust. Linux needs GTK 3, Ayatana AppIndicator, librsvg and
libxkbcommon-x11 development packages. Windows needs MSVC; macOS needs Xcode
command-line tools.

```sh
cargo run --package draugr-time
cargo build --package draugr-time --release
```

The application is `target/release/draugr-time` (`.exe` on Windows). Press
**Start recording** to show recorded activity. Closing the window keeps the
tray and tracking; Quit stops the application. Settings can choose another
data folder for the next launch. Existing history stays in its original folder.

New installations use the OS local-data directory under `DraugrTime`. Existing
installations continue using a legacy `DagdaTime` folder when the new folder
does not exist. `DRAUGR_TIME_DATA_DIR` overrides the data folder; the old
`DAGDATIME_DATA_DIR` variable is also accepted for compatibility.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 -m unittest discover -s tests
```

CI builds and launches the native desktop on Linux, Windows and macOS.
`--smoke-test` renders synthetic history, `--runtime-test` records mock samples,
and `--lifecycle-test` closes and reopens the window while recording continues.
These diagnostic modes use temporary history and exit automatically. Normal
launches have no demo mode. `--autostart-test` checks the OS startup provider
and restores its previous enabled state; run it only in an isolated account.

Optional watcher diagnostics:

```sh
cargo run --package draugr-time-daemon --bin tracker -- debug
```

X11 integration tests modify the active-window property. Use an isolated display:

```sh
env -u WAYLAND_DISPLAY XDG_SESSION_TYPE=x11 xvfb-run -a cargo test --package draugr-time-watchers --package draugr-time-daemon -- --ignored --test-threads=1
```

Manual checks: switch between applications and Unicode titles, idle and return,
pause/resume from window and tray, right-click the tray while the window is open,
close/reopen, Quit, and select a data folder then restart. Verify macOS
Accessibility permissions, Windows protected processes, and real compositor
bridges on Wayland. See [wayland.md](wayland.md), [desktop.md](desktop.md),
[PLAN.md](../PLAN.md), and [decisions.md](decisions.md).

All application network traffic stays on localhost. No telemetry.
