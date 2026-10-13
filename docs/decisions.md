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
