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
