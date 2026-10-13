# DagdaTime

A local, private desktop time tracker, licensed under GPLv3. The project is
currently at M0: an empty Tauri 2 window and a Rust workspace. Tracking and
storage are not implemented yet. The application makes no external network
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
