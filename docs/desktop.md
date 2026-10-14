# Draugr Time desktop

Draugr Time uses Iced and its native tiny-skia renderer. One executable contains
tracking, SQLite, the localhost API, the Day view and the system tray.

![Draugr Time in the light theme](screenshots/draugr-time-light.png)

![Draugr Time in the dark theme](screenshots/draugr-time-dark.png)

Screenshots use generated sample data. Normal launches contain no demo mode;
fixtures are reserved for tests and explicit diagnostics.

Run `cargo run --release --package draugr-time`. Press **Start recording** to
collect real activity. The Day view shows applications, documents, computer
state and tag tracks; select a range to see exact durations. **Theme** cycles
System, Light and Dark. Closing the window keeps tracking in the tray; Quit
stops the whole application. Only one desktop process can run per user.

In the **Applications** summary, click an application to expand its documents
or browser page titles, ranked by time spent in the selected range (or the
whole day). Repeated visits are combined; click again to collapse the list.
Browser pages currently use recorded window titles; exact URLs require the
browser integration planned for M9.

Settings can select the next-launch data folder and enable **Start Draugr Time
when I log in**. Startup launches with `--autostart --minimized`. After moving
or renaming the executable, toggle startup again to register its current path.

Linux uses GTK/AppIndicator for the tray and supports X11 and Wayland windows.
Windows and macOS use native tray and icon providers. Real focus, idle,
permissions, tray actions, folder selection and startup registration need
manual verification on each OS; CI checks builds and diagnostic launches.

Live tracker ends are buffered for up to 30 seconds; UI and localhost API queries
include the in-memory overlay. Window/AFK changes, pause and Quit flush immediately.
A crash can lose the most recent 30 seconds of stable activity. Closing the main
window keeps tracking; Linux/Windows no longer need GUI timer ticks while hidden.
macOS still samples on the main thread once per second.

After updating the Wayland bridge files, reload the GNOME extension / KWin script.
Manually verify: switch windows, edit a focused window's title, leave focus unchanged
for over 30 seconds, restart the tracker, then disable the bridge. Focus/title must
update promptly, restart must recover (KDE within 30 seconds), and disabling must
show an unsupported backend after three seconds. On Windows verify steady focus,
rapid app switching, closing/reopening processes and hide-to-tray recording. On
macOS verify hidden-window recording and Accessibility permission errors. In the
native UI verify zoom/pan, theme and track visibility after live updates: the static
grid and dynamic events must stay aligned.

M4.5 records computer state independently of app/title availability. Computer
shows Active, AFK, Sleep, Locked, Paused, Tracker stopped and No recording.
Unobserved identifies an actual observation failure; inspect its tooltip or
Settings → Status & permissions for the reason and OS evidence source. With a
valid idle sample and denied title access, active/AFK time is retained and the
Applications/Documents track shows the diagnostic. `/status` includes `computer`
and `computer_capabilities`; `/events?bucket=computer&from=…&to=…` includes the
same evidence and reason in each event's data (timestamps are UTC milliseconds).

Sleep/lock monitoring is independent of the window backend. Linux uses logind
(the desktop must report LockedHint), Windows uses WTS/power notifications, and
macOS uses main-thread NSWorkspace sleep/wake notifications. macOS screen locking
and historical physical-off reconstruction are explicitly unavailable. A shutdown
request, a reboot, or missing heartbeats do not establish how long power was off;
those gaps remain No recording. Reinstalling a Wayland bridge is not needed for
this system-bus lifecycle collector. See `computer-state-research.md` for sources,
per-platform limits, permissions and manual sleep/wake/lock/reboot checks.

### Tags (M5)

Select a time range and press Enter or **Tag selection**. Type a comma-separated
hierarchy, such as `Client, Website, Design`; suggestions include recent and
existing tags. Add a note if needed. When the range overlaps existing blocks,
choose **Split existing blocks around selection** (keeps outside time) or
**Replace intersecting blocks** (removes those blocks entirely).

On the Tags track, double-click a block to edit its tag/note or delete it. Drag
its middle to move it; drag either edge to resize it. Shift-drag continues to
select time instead. The editor also accepts local date/time with timezone for
precise boundaries, including ranges across midnight. Ctrl/Cmd+Z or **Undo tag
action** restores the last action (up to 20 in this process). Undo refuses to
replace newer edits made through the localhost API.

The Tags screen shows hierarchy paths and supports rename, color, archive,
merge and confirmed subtree deletion. Archived tags retain historical blocks.
The tray offers **Tag last 5 minutes**, **Tag last 15 minutes**, and **Tag last
hour**. Recording must be started before saving tags.

After returning from at least 15 minutes of observed AFK, a non-modal prompt
shows the full away interval, recent tags and Ignore. Sleep, shutdown, locking,
pauses and recording gaps are excluded. Settings → While you were away changes
or disables the threshold. A qualifying return while the window is hidden opens
it with the prompt; viewing a historical day does not stop AFK detection.

Manual verification on Windows/macOS/Linux (X11 and Wayland): select and tag a
range, check suggestions and Enter/Escape, double-click and drag a tag's middle
and edges, then undo. Verify an overnight block retains its full duration when
moved from either day. Check overlap Split/Replace, manager merge/archive/delete,
tray quick tags while hidden, and a ≥15-minute AFK return. Sleep/lock/pause should
not prompt. Restart after changing the prompt threshold and check persistence.
