# PLAN.md — Open-source, cross-platform ManicTime alternative

> Working name: Draugr Time (do **not** use "ManicTime" in the name — it is a trademark).

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
2. **Desktop app with a ManicTime-style Day view**: timeline with tracks (Tags, Computer usage, Applications, Documents/URLs), range selection and a summary panel (see §5).
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
| GUI | **Iced** (Rust, tiny-skia) | Native cross-platform UI and canvas, no webview |
| Storage | **SQLite** (rusqlite or sqlx) | Local, single file, easy backups |
| Timeline | Canvas/SVG (vis-timeline or custom d3) | Needs zoom and range selection |
| CI | GitHub Actions, matrix windows/macos/ubuntu | Agent can verify builds on all OSes |
| License | GNU GPLv3 |

---

## 3. Architecture

One desktop executable contains the Iced window, native watchers, background
tracking, SQLite storage and the localhost API. No separate daemon executable
is required. Closing the window hides it to the tray and keeps tracking; Quit
stops the entire application. The CLI is an optional developer diagnostic.
Data defaults to the OS application-data directory. Settings can select another
folder for the next launch; existing history remains in its original folder.

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

## 5. Application UI (modeled on ManicTime)

The desktop app is the product. Background tracking is embedded in the desktop process; the CLI is diagnostic. The UI follows ManicTime's proven model: **one main "Day view" where you see the timeline, select time and tag it**, plus a few supporting screens. Copy the interaction model, not the look or branding.

### Design principles
- **Desktop-dense, not mobile-airy.** Lots of information on one screen, compact rows, small margins.
- **Keyboard-first.** ←/→ previous/next day, `T` today, `+`/`-` zoom, `Enter` tag the selection, `Ctrl/Cmd+F` search.
- **Selection drives everything.** Selecting a range on any track updates the summary panel and enables tagging.
- **Never silent.** If tracking is paused, blocked by permissions or unsupported (Wayland), it is visible on screen and in the tray.
- Light and dark themes following the OS. Local time zone in the UI; UTC everywhere else.

### Window layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ ◀  Tue, 13 Oct 2026  ▶  [Today]  [📅]       🔍 Search   ● Tracking ▾│  toolbar
├────────┬─────────────────────────────────────────────────────────────┤
│ Day    │  08    09    10    11    12    13    14    15    16    17  │  time ruler
│ Week   │ Tags      ██Project A██      ░░░░  ██Client B████          │
│ Reports│ Computer  ████████████  ▒▒▒  ███████████████████  ▒▒       │  active / away / off
│ Tags   │ Apps      ██▓▓██░░████        ██▓▓▓▓██████░░████            │  colored per app
│ Rules  │ Documents ▏▏▏ ▏▏▏▏ ▏ ▏▏         ▏▏▏▏ ▏▏▏ ▏▏▏▏               │  titles / URLs
│        ├─────────────────────────────────────────────────────────────┤
│        │ Selection 10:12–11:47 (1h 35m)        [Tag selection ⏎]     │
│        │ ┌ Tags ┬ Applications ┬ Documents ┐                          │  summary panel
│        │ │ Project A   1h 10m  ██████████  74%                         │
│ ⚙      │ │ (untagged)    25m   ███         26%                         │
└────────┴─────────────────────────────────────────────────────────────┘
```

### Screens

**1. Day view (main screen, ManicTime's "Day view")**
- Toolbar: previous/next day, Today, date picker, search, tracking status pill (Tracking / Paused / Away / Error) with a dropdown: Pause 15 min / 1 h / until resumed.
- Timeline tracks, top to bottom: **Tags**, **Computer usage** (active / away / off), **Applications** (stable color per app), **Documents** (window titles, URLs from M9). Track order and visibility configurable.
- Zoom from whole day down to minutes; scroll horizontally; hover shows app, title, start–end, duration.
- Click a block to select it; drag to select any range; Shift-click to extend. Double-click an app block selects its continuous span.
- **Summary panel** under the timeline with tabs *Tags / Applications / Documents*: duration and share for the current selection, or the whole day if nothing is selected. Clicking a row highlights its blocks on the timeline.
- Day totals in the corner: active time, away time, tagged vs untagged.

**2. Tagging (popup on the Day view)**
- `Enter` or "Tag selection" opens a small popup: text field with autocomplete from existing tags and recent ones.
- Hierarchical tags typed with commas, as in ManicTime: `Client B, Website, Design`.
- Optional note. Tagging an already-tagged range asks: replace, or split around it.
- Edit/move/resize/delete tag blocks directly on the Tags track; undo with `Ctrl/Cmd+Z`.

**3. "While you were away" prompt**
- When the user returns after away time ≥ threshold (default 15 min), a small non-modal window: *"You were away 10:30–11:15 (45 min). What were you doing?"* → quick tag (recent tags as buttons), ignore, or "don't ask for gaps shorter than…".
- Answers create tag ranges on the away period (meetings, calls, lunch).

**4. Week view**
- Seven days stacked as compact timelines; click a day to open it in the Day view. Weekly totals per tag.

**5. Reports**
- Date range (day / week / month / custom), group by *tag / application / document*, optional filter by tag.
- Table with durations + one bar chart. "Timesheet" layout: tags × days grid with daily totals.
- Export CSV / JSON; copy as text.

**6. Tags manager**
- Tree of tags (client → project → task), rename (updates history), merge, color, archive, delete with confirmation showing affected time.

**7. Auto-tag rules**
- List of rules: field (app / title / URL), pattern, tag, priority, enabled.
- **Live preview**: "this rule would tag 3h 20m in the last 7 days", with sample matches.
- "Apply to history" for a date range, with a confirmation and undo.

**8. Settings**
- *Tracking*: away threshold, away prompt on/off, start with system, start minimized to tray.
- *Privacy*: excluded apps (not recorded at all), title masking per app, pause shortcuts, "delete data for range".
- *Data*: database location, backup now / automatic backups, import (ActivityWatch), export.
- *Status & permissions*: per-backend status (Windows / macOS Accessibility / X11 / Wayland GNOME/KDE/wlroots / browser extension) with a fix-it hint for each problem.
- *About*: version, license, link to the repo.

**9. First-run onboarding (3–4 steps)**
- What is tracked and that it never leaves the machine → grant permissions (macOS Accessibility; on Wayland: install the GNOME extension / KWin script or explain wlroots) → optional browser extension → done, open Day view.

**10. System tray / menu bar**
- Icon shows state: tracking / paused / away / error.
- Menu: open Draugr Time, current activity, today's active time, **tag the last N minutes** (quick tag), Pause 15 min / 1 h / until resumed, Settings, Quit.
- Closing the main window keeps tracking in the background.

### Empty and error states (required on every screen)
- No data yet → short explanation + "tracking started at 09:41".
- Daemon not running → banner with "Start tracking" button.
- Backend unsupported or permission missing → banner linking to Settings → Status & permissions.

### UI tech decisions (to be recorded in `docs/decisions.md` in M3.5)
- Framework: Iced in Rust with the tiny-skia renderer; no remote assets.
- Timeline rendered on the native Iced **canvas**; native controls for everything else.
- UI uses in-process tracking commands and snapshots from the runtime. The UI never issues SQL directly; the localhost API remains available for local integrations.
- Rust tests for summary math and selection; native smoke/runtime/lifecycle checks with isolated fixture data.

---

## 6. Milestones

Each milestone = one focused AI session. Do not start the next until the acceptance criteria pass.

### M0 — Skeleton
- Cargo workspace: `core`, `watchers`, `daemon`, `app` (Iced).
- CI matrix on 3 OSes: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- ✅ Empty Iced window builds in CI on Windows, macOS and Linux.

### M1 — Storage & heartbeat logic
- SQLite schema + migrations; `heartbeat(bucket, data, ts, pulsetime)`.
- ✅ Unit tests: identical events merge; window change splits; gap > pulsetime splits.

### M2 — Watchers: Windows, X11, macOS
- Trait implementations + `MockWatcher`.
- ✅ `tracker debug` CLI prints the current window and idle seconds every second.

### M3 — Daemon & local API
- Polling loop; AFK logic (idle > 5 min → `away`, configurable).
- REST: `GET /events?bucket&from&to`, CRUD for `/tags`, `/tag_ranges`, `/rules`, plus `GET /status` (tracking state, per-backend status and errors) and `POST /pause` / `POST /resume`.
- ✅ Integration test with `MockWatcher`: a simulated 10-minute scenario produces the expected events.
- Wayland backends: GNOME (D-Bus + extension), KDE (KWin script), wlroots (IPC).
- ✅ Wayland graceful fallback: if no backend is available, `/status` reports it and the UI (M3.5) clearly says so instead of silently recording nothing.

### M3.5 — App shell
- Use Iced and the Rust desktop build (see §5 "UI tech decisions"); record architecture decisions in `docs/decisions.md`.
- Main window with sidebar navigation (Day, Week, Reports, Tags, Rules, Settings) and placeholder screens.
- Toolbar with date navigation and the tracking status pill; Pause / Resume.
- **Tray / menu bar icon** with state and the menu from §5.10 (quick tag may be disabled until M5). Closing the window keeps tracking.
- Settings → Status & permissions screen showing live `/status`.
- Empty and error states: daemon not running, backend unsupported, permission missing.
- ✅ App launches on all 3 OSes in CI; on fixture/mock daemon data the status pill, tray state and Status screen reflect tracking / paused / error; pause from tray is reflected in the window and vice versa.

### M4 — Day view & timeline
- Day view from §5.1: tracks Tags, Computer usage, Applications, Documents; zoom (day ↔ minutes), scroll, hover details; stable per-app colors.
- Range selection (click, drag, Shift-click) and the **summary panel** (Tags / Applications / Documents tabs) for the selection or the whole day.
- Day totals; keyboard navigation between days.
- ✅ Renders fixture data with 5 000+ events smoothly; selection math (durations, shares) covered by tests; summary updates on selection change.

### M5 — Tags (core feature)
- Tagging popup from §5.2: autocomplete, comma-separated hierarchy, note; replace/split on overlap; edit, resize, move, delete on the Tags track; undo.
- **"While you were away" prompt** from §5.3.
- Tags manager screen (§5.6).
- Tray "tag the last N minutes" enabled.
- ✅ Tested: total time per tag per day is correct, including overlaps, splits and day boundaries; away prompt creates the right tag range.

### M6 — Auto-tagging rules
- Regex rules on app/title/url with priorities; "apply to history" action.
- Rules screen (§5.7) with live preview and undo for "apply to history".
- ✅ Tests for rule matching and priority resolution; preview totals match the applied result.

### M7 — Week view, reports & export
- Week view (§5.4) and Reports screen (§5.5) with timesheet layout.
- CSV/JSON export; ActivityWatch import (Settings → Data).
- ✅ Export → import round-trip test; report totals equal the sum of Day view totals for the same range.

### M8 — Reserved
- Wayland scope and acceptance criteria moved to M3 at the user's request.

### M9 — Browser extension
- Firefox + Chromium; sends active tab URL/title to the daemon.
- ✅ URLs appear on the Documents track; extension state is shown in Settings → Status & permissions.

### M10 — Polish & distribution
- First-run onboarding (§5.9).
- Settings completed (§5.8): autostart, privacy controls (excluded apps, title masking, delete range), backups.
- Installers: MSI (Windows), DMG (macOS), AppImage/Flatpak (Linux).
- ✅ Fresh install on each OS goes through onboarding to a working Day view without touching a terminal.

### Later
Screenshots track (as in ManicTime), DB encryption, multi-device sync (CRDT or self-hosted server).

---

## 7. Risks

- **macOS permissions** (Accessibility / Screen Recording): needs clear onboarding, otherwise titles come back empty.
- **Wayland**: the most labor-intensive part; scheduled in M3 before the timeline UI.
- **Timeline performance**: thousands of events per day; render on canvas and aggregate when zoomed out.
- **DB growth**: index on `start_ts`, optional aggregation of old data.
- **Privacy**: the tracker sees everything. Nothing leaves the machine by default — state this in the README.
