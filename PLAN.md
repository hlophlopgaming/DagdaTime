# PLAN.md — Open-source, cross-platform ManicTime alternative

> Working name: DagdaTime (do **not** use "ManicTime" in the name — it is a trademark).

## 0. Positioning

ActivityWatch already exists as an open-source cross-platform tracker (MPL-2.0). What it lacks compared to ManicTime:

- a proper **timeline** with manual time tagging (projects/tasks),
- good "how much did I work on X" reports,
- solid **Wayland** support.

Decision: build our own project, but keep the **event format compatible** with ActivityWatch so we can import its data and reuse its browser extensions.

---

## 1. Scope

### MVP
1. Automatic tracking: active application, window title, computer state (active / idle / off).
2. **Timeline** with tracks: Computer usage, Applications, Documents/URLs, Tags.
3. **Tags**: select a range on the timeline → assign a project/task. This is the core ManicTime feature.
4. Auto-tagging rules (regex on app / title / URL).
5. Reports and stats for day/week; CSV/JSON export.
6. Fully local, offline, private by default.

### Out of MVP
Screenshots, multi-device sync, server edition, mobile clients.

---

## 2. Tech stack

| Layer | Choice | Reason |
|---|---|---|
| Core / daemon | **Rust** | Single binary per OS, low overhead, good OS bindings |
| GUI | **Tauri 2** + Svelte (or React) | Cross-platform, small footprint, web UI suits a timeline |
| Storage | **SQLite** (rusqlite or sqlx) | Local, single file, easy backups |
| Timeline | Canvas/SVG (vis-timeline or custom d3) | Needs zoom and range selection |
| CI | GitHub Actions, matrix windows/macos/ubuntu | Agent can verify builds on all OSes |
| License | GNU GPLv3 |

---

## 3. Architecture

```
┌──────────────┐   heartbeats   ┌───────────────┐   HTTP/IPC   ┌──────────┐
│  Watchers    │ ─────────────▶ │  Core daemon  │ ◀──────────▶ │ Tauri UI │
│  (per-OS)    │                │ storage + API │              └──────────┘
└──────────────┘                └───────────────┘
                                        ▲
           browser extension ── localhost ┘
```

- The **daemon** runs in the background (autostart). The UI can be closed without stopping tracking.
- **Watchers** live behind traits, with a mock implementation for tests:

```rust
pub trait WindowWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>>;
}

pub trait IdleWatcher {
    fn idle_seconds(&self) -> Result<u64>;
}

pub struct WindowInfo {
    pub app: String,       // executable / bundle id
    pub title: String,
    pub pid: Option<u32>,
}
```

### Data model (heartbeat approach, ActivityWatch-compatible)

```sql
CREATE TABLE events (
    id        INTEGER PRIMARY KEY,
    bucket    TEXT NOT NULL,      -- 'window' | 'afk' | 'web'
    start_ts  INTEGER NOT NULL,   -- unix ms, UTC
    end_ts    INTEGER NOT NULL,
    data      TEXT NOT NULL       -- JSON
);
CREATE INDEX idx_events_bucket_start ON events(bucket, start_ts);

CREATE TABLE tags (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    color     TEXT,
    parent_id INTEGER REFERENCES tags(id)
);

CREATE TABLE tag_ranges (
    id       INTEGER PRIMARY KEY,
    tag_id   INTEGER NOT NULL REFERENCES tags(id),
    start_ts INTEGER NOT NULL,
    end_ts   INTEGER NOT NULL,
    note     TEXT
);

CREATE TABLE rules (
    id       INTEGER PRIMARY KEY,
    field    TEXT NOT NULL,       -- 'app' | 'title' | 'url'
    pattern  TEXT NOT NULL,       -- regex
    tag_id   INTEGER NOT NULL REFERENCES tags(id),
    priority INTEGER NOT NULL DEFAULT 0
);
```

**Heartbeat merging:** watchers send a heartbeat every N seconds. If `data` equals the last event's data in the same bucket and the gap is `< pulsetime`, extend `end_ts`; otherwise insert a new event.

---

## 4. Platform specifics

| OS | Active window | Idle time |
|---|---|---|
| **Windows** | `GetForegroundWindow` + `GetWindowTextW` + `GetWindowThreadProcessId` → exe name | `GetLastInputInfo` |
| **macOS** | `NSWorkspace.frontmostApplication`; title via Accessibility API (requires permission) | `CGEventSourceSecondsSinceLastEventType` |
| **Linux X11** | `_NET_ACTIVE_WINDOW` + `_NET_WM_NAME` (x11rb) | XScreenSaver extension |
| **Linux Wayland** | No universal API. GNOME: shell extension exposing D-Bus; KDE: KWin script; Sway/Hyprland: compositor IPC | `ext-idle-notify-v1`, or `org.freedesktop.ScreenSaver` / logind `IdleHint` |

**Browser URLs:** window titles are not enough, so a browser extension (Firefox + Chromium) sends the active tab's URL/title to the daemon on `localhost`.

---

## 5. Milestones

Each milestone = one focused AI session. Do not start the next until the acceptance criteria pass.

### M0 — Skeleton
- Cargo workspace: `core`, `watchers`, `daemon`, `app` (Tauri).
- CI matrix on 3 OSes: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- ✅ Empty Tauri window builds in CI on Windows, macOS and Linux.

### M1 — Storage & heartbeat logic
- SQLite schema + migrations; `heartbeat(bucket, data, ts, pulsetime)`.
- ✅ Unit tests: identical events merge; window change splits; gap > pulsetime splits.

### M2 — Watchers: Windows, X11, macOS
- Trait implementations + `MockWatcher`.
- ✅ `tracker debug` CLI prints the current window and idle seconds every second.

### M3 — Daemon & local API
- Polling loop; AFK logic (idle > 5 min → `away`, configurable).
- REST: `GET /events?bucket&from&to`, CRUD for `/tags`, `/tag_ranges`, `/rules`.
- ✅ Integration test with `MockWatcher`: a simulated 10-minute scenario produces the expected events.
- Wayland backends: GNOME (D-Bus + extension), KDE (KWin script), wlroots (IPC).
- ✅ Wayland graceful fallback: if no backend is available, the UI clearly says so instead of silently recording nothing.

### M4 — Timeline UI
- Tracks: Computer, Applications, Documents/URLs, Tags. Zoom (day ↔ hour), hover details.
- ✅ Renders 3+ tracks from fixture data; zoom and scroll work.

### M5 — Tags (core feature)
- Drag-select a range → assign tag; edit/delete ranges; nested tags.
- ✅ Tested: total time per tag per day is correct, including overlaps and day boundaries.

### M6 — Auto-tagging rules
- Regex rules on app/title/url with priorities; "apply to history" action.
- ✅ Tests for rule matching and priority resolution.

### M7 — Reports & export
- Day/week summaries by application and by tag; CSV/JSON export; ActivityWatch import.
- ✅ Export → import round-trip test.

### M8 — Reserved
- Wayland scope and acceptance criteria moved to M3 at the user's request.

### M9 — Browser extension
- Firefox + Chromium; sends active tab URL/title to the daemon.
- ✅ URLs appear on the Documents/URLs track.

### M10 — Polish & distribution
- Tray icon, autostart, settings, privacy controls (excluded apps, title masking), DB backup.
- Installers: MSI (Windows), DMG (macOS), AppImage/Flatpak (Linux).

### Later
Screenshots, DB encryption, multi-device sync (CRDT or self-hosted server).

---

## 6. Risks

- **macOS permissions** (Accessibility / Screen Recording): needs clear onboarding, otherwise titles come back empty.
- **Wayland**: the most labor-intensive part; scheduled in M3 before the timeline UI.
- **DB growth**: index on `start_ts`, optional aggregation of old data.
- **Privacy**: the tracker sees everything. Nothing leaves the machine by default — state this in the README.
