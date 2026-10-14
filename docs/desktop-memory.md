# M5.5 desktop memory checks

Measured on 2026-10-14 on Linux/Wayland, tiny-skia, release builds. Baseline was
the pre-change `target/release/draugr-time` saved before rebuilding. Each process
used isolated data/config directories; no personal history was loaded. The
`--benchmark` fixture contains 5,000 window events, opens a 1280×820 window and
closes it after seven seconds. Read `/proc/<pid>/smaps_rollup` at four seconds
(window open), nine and eleven seconds (window closed). Separate `--minimized`
launches were sampled at four seconds without starting recording.

| Scenario | Before PSS | After PSS | Before RSS | After RSS |
|---|---:|---:|---:|---:|
| Tray only, no window opened | 32.33 MiB | 11.43 MiB | 59.16 MiB | 15.09 MiB |
| Window open, 5,000 events | 45.44 MiB | 31.93 MiB | 72.43 MiB | 36.19 MiB |
| After closing window | 36.44 MiB | 17.48 MiB | 63.42 MiB | 21.75 MiB |

PSS decreased about 65% for tray-only startup and 52% after closing the fixture
window. At nine and eleven seconds the post-close values were identical. These
are observations on one machine, not a promised memory bound or an isolated
measurement of each change. Earlier repeat runs had post-close PSS around
18.7 MiB; resident shared-library pages and system activity affect PSS.

Closing does not restore the exact cold-start baseline: the final run retained
about 6 MiB more PSS than tray-only startup (private dirty memory: 4.81 vs
2.31 MiB). Code/font/runtime caches and preserved editing/undo state may remain.
Existing tag undo snapshots still cover whole tag history and are kept for up to
20 actions; this milestone preserves undo rather than silently discarding it.
No heaptrack attribution or minute-long settling measurement was performed.
One repeated native Wayland run timed out in the runtime diagnostic (30 seconds).
A diagnostic rerun and three further isolated runtime runs passed; the timeout
was not reproduced and no stack of the stalled process was captured. Its cause
is not established; real-session stability still needs monitoring.

Verified locally:

- Linux dependency tree and dynamic linkage have no GTK/GDK/Pango/Cairo or
  libappindicator; ksni and ashpd use the existing zbus/async-io stack.
- Installed KDE application icons and preview-process icons resolve to raw local PNG/SVG images (both platform tests passed).
- Native Wayland smoke test renders 5,000 fixture events.
- Runtime fixture writes to isolated SQLite; lifecycle fixture closes the window,
  keeps recording without day reloads, then reopens and displays the new samples.
- Real ksni D-Bus menu dispatch works without GUI ticks on a private session bus.
  Run `dbus-run-session -- cargo test --package draugr-time native_ksni_menu_dispatches_without_gui_ticks -- --ignored --test-threads=1`.
- Rust regression tests cover releasing buffers, stale results, lazy/shared search,
  tail replacement, raw icon handles, day-scoped tag loads, original interval
  bounds, outside-day overlap, whole-history delete totals and v2→v3 migration.

Manual verification remaining:

- Linux X11 and Wayland: right-click the tray while the window is hidden, test
  Open, Settings, quick tags, Pause, Resume and Quit. Disable the SNI host and
  verify visible-window fallback. GNOME requires an SNI/AppIndicator extension.
- Linux folder selection: with the desktop's xdg-desktop-portal backend installed,
  open Settings → Choose folder, select an absolute path, restart and verify the
  selected data directory. Cancel must leave the preference unchanged.
- Windows/macOS: build and run, verify raw PNG application icons, close/reopen,
  unsaved tag edits and undo, tray menus and native folder selection. macOS also
  needs a check that background sampling and permissions still work when hidden.

For longer measurements, open and close the normal application on an isolated
fixture database and sample smaps_rollup after a minute. Compare PSS, not only RSS;
check preserved undo separately after editing a large history.
