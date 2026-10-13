<div align="center">

# DagdaTime

### Open-source, cross-platform ManicTime alternative

**Automatic time tracking that stays on your computer.**<br>
See where your day went on a timeline, tag time to projects, get honest reports. Free and private.

[![CI](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DagdaTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-orange)
![No telemetry](https://img.shields.io/badge/telemetry-none-success)
[![GitHub stars](https://img.shields.io/github/stars/hlophlopgaming/DagdaTime?style=social)](https://github.com/hlophlopgaming/DagdaTime/stargazers)

**English** · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

---

> [!IMPORTANT]
> **DagdaTime is in early development.** Native window and idle tracking already work on Windows, macOS and Linux (X11). The timeline, tags and reports are next on the [roadmap](#roadmap).
> ⭐ **Star** or 👀 **Watch** the repo to follow progress and get notified about the first release.

## Why DagdaTime?

You open your laptop at 9, and suddenly it's 18:00. Where did the day go?

**ManicTime** answers that question well, but it is closed source and its most useful features sit behind a paid license. **DagdaTime** aims to be the same kind of tool, built in the open:

- 🕒 **Automatic tracking.** Records the active app, window title and whether you were at the computer. No timers to start or forget.
- 📊 **Timeline view.** Your whole day on parallel tracks: computer usage, applications, documents & websites, tags.
- 🏷️ **Tag time, not tasks.** Drag across the timeline and assign the range to a project or client. Rules can tag it automatically.
- 🔒 **Private by default.** Everything lives in a local SQLite file. No account, no cloud, no telemetry, no network calls.
- 💻 **Truly cross-platform.** Windows, macOS and Linux from one Rust codebase, including native **Wayland** support (GNOME, KDE, Sway/Hyprland), not just XWayland.
- 🆓 **Free forever.** GPL-3.0. No "Pro" tier.

## DagdaTime vs ManicTime vs ActivityWatch

|  | **DagdaTime** | ManicTime | ActivityWatch |
|---|---|---|---|
| Open source | ✅ GPL-3.0 | ❌ Proprietary | ✅ MPL-2.0 |
| Price | ✅ Free | Free edition, paid Pro | ✅ Free |
| Data stays on your machine | ✅ | ✅ | ✅ |
| Timeline with manual tagging of time ranges | 🚧 Planned (M5) | ✅ | Rule-based categories |
| Auto-tagging rules (app / title / URL) | 🚧 Planned (M6) | ✅ | ✅ |
| Native Wayland backends | ✅ GNOME/KDE/wlroots | — | Limited |
| Import from ActivityWatch | 🚧 Planned (M7) | ❌ | — |

DagdaTime keeps its event format compatible with ActivityWatch, so you can bring your history with you.

## Roadmap

- [x] **M0** Rust workspace, Tauri 2 desktop shell, CI on Windows / macOS / Linux
- [x] **M1** SQLite storage with migrations and heartbeat event merging
- [x] **M2** Native watchers: active window + idle time on Windows, macOS, Linux X11
- [x] **M3** Background daemon, local API, AFK detection, Wayland (GNOME, KDE, wlroots)
- [x] **M3.5** App shell: main window, navigation, tracking status, tray icon with pause/resume
- [x] **M4** Day view: timeline, range selection, summary panel (like ManicTime)
- [ ] **M5** Tags: select a range on the timeline → assign a project
- [ ] **M6** Auto-tagging rules
- [ ] **M7** Reports, CSV/JSON export, ActivityWatch import
- [ ] **M9** Browser extension for URLs (Firefox, Chromium)
- [ ] **M10** Tray icon, autostart, privacy controls, installers (MSI, DMG, AppImage/Flatpak)
- [ ] **Later** Screenshots, database encryption, multi-device sync

Full details: [PLAN.md](PLAN.md) · Architecture decisions: [docs/decisions.md](docs/decisions.md)

DagdaTime runs as one desktop executable with embedded tracking and a tray. Data defaults to the system application-data folder; Settings lets you select another folder for the next launch.

## Try it (developers)

There are no installers yet. You can build from source and run the watcher debug tool, which prints the active window and idle time once per second:

```sh
git clone https://github.com/hlophlopgaming/DagdaTime.git
cd DagdaTime
npm ci --prefix app/frontend
npm --prefix app/frontend run build
cargo run --package dagdatime-app
```

Build requirements and platform notes: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Privacy

A time tracker sees everything you do, so this is non-negotiable:

- All data is stored locally in a single SQLite file that you own.
- DagdaTime makes **no external network calls** and has **no telemetry**.
- Planned: excluded apps, window-title masking, optional database encryption.

## FAQ

<details>
<summary><b>Is there a free, open-source alternative to ManicTime?</b></summary>

That is exactly what DagdaTime is built to be: automatic tracking, a timeline and time tagging, free and GPL-licensed, on Windows, macOS and Linux.
</details>

<details>
<summary><b>Does it work on Linux and Wayland?</b></summary>

Yes, Linux is a first-class platform. X11 already works. Wayland backends for GNOME, KDE, Sway and Hyprland are implemented. GNOME/KDE require a local compositor integration; see [Wayland setup](docs/wayland.md). Real compositor behavior needs manual verification.
</details>

<details>
<summary><b>Is it an alternative to RescueTime, Timing or WakaTime too?</b></summary>

Partly. Like RescueTime and Timing it tracks your computer usage automatically, but everything stays local and there is no subscription. It is not limited to code editors like WakaTime.
</details>

<details>
<summary><b>Can I import my ManicTime or ActivityWatch data?</b></summary>

ActivityWatch import is planned for M7. ManicTime import is under consideration: open an issue if you need it.
</details>

<details>
<summary><b>Does it upload my data anywhere?</b></summary>

No. There is no server, account or analytics. Multi-device sync may come later, and it will be opt-in.
</details>

## Contributing

Contributions are welcome, especially from people who know:

- **Wayland** compositors (GNOME Shell extensions, KWin scripts, wlroots IPC)
- **macOS** Accessibility APIs
- **Timeline UI** design and data visualization

Open an issue to discuss an idea, or pick a milestone from [PLAN.md](PLAN.md). The project is developed with AI coding agents; their working rules are in [AGENTS.md](AGENTS.md).

If DagdaTime looks useful to you, **a star helps other people find it.** ⭐

## License

[GPL-3.0](LICENSE)

<sub>ManicTime, RescueTime, Timing and WakaTime are trademarks of their respective owners. DagdaTime is an independent project and is not affiliated with them.</sub>
