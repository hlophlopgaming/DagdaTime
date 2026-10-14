# M4.5 local computer-state evidence research

Date: 2026-10-14. No network was used. Linux system documentation and installed generated Apple/Microsoft bindings were inspected. API presence in bindings establishes availability, **not** runtime behavior, permissions or retained-history semantics on another OS. Unverified mechanisms below are explicitly candidates.

## Capability table

| Platform | Sleep / resume | Shutdown / boot | Lock / unlock | Idle | Permissions, retention and restart |
|---|---|---|---|---|---|
| Linux X11 | Verified interface: logind `PrepareForSleep(bool)`; `PreparingForSleep` initial snapshot. | Verified interface: `PrepareForShutdown(bool)`; recent versions include metadata type. A preparation signal is not proof of completed poweroff. Kernel boot identity proves another boot, not when the previous system powered off. | Own logind Session `LockedHint`, supplied by desktop; false is not reliable if desktop does not implement reporting. `Lock`/`Unlock` signals are requests, not confirmations. | Existing XScreenSaver watcher; desktop logind `IdleHint` is only a hint with desktop-specific threshold. | System D-Bus readable subject to installed policy; delay inhibitor is polkit-controlled. Live signals have no replay. Journal history may require group/ACL access, may be volatile or rotated. |
| Linux Wayland | Same logind system interface; independent of compositor window protocol. | Same limits as X11. | Same LockedHint dependency; compositor/screen-locker support must be manually verified. No universal window/lock evidence may be assumed. | Existing GNOME/ext-idle-notify implementations; lack of focus must not invalidate idle. | Session discovery must select this user's graphical session, never arbitrary first session. GetSessionByPID can fail for user services outside a session. |
| Windows | API presence verified: `PowerRegisterSuspendResumeNotification`, `WM_POWERBROADCAST`, `PBT_APMSUSPEND`, `PBT_APMRESUMEAUTOMATIC`. Native semantics and Modern Standby delivery need validation. | `WM_QUERYENDSESSION` / `WM_ENDSESSION` exist, but session ending is not evidence of physical off. Retained System Event Log is a candidate; no event-ID mapping verified locally. | `WTSRegisterSessionNotification` and `WTSQuerySessionInformationW(WTSSessionInfoEx)` exist. Use own session; query flags need native validation. | Existing `GetLastInputInfo`; current-session input only. | Read own session without requesting broad privileges; handle failure. Event Log access, rotation, Fast Startup, canceled shutdown and emergency power loss need native checks. No validated historical reconstruction in this local research. |
| macOS | Verified bindings expose NSWorkspaceWillSleepNotification and NSWorkspaceDidWakeNotification with Apple official reference URLs. AppKit notification center/run loop required by existing architecture. | NSWorkspaceWillPowerOffNotification exists; records intent, not completion. Unified log / pmset history and kern.boottime are candidates, not validated here. | NSWorkspaceSessionDidResignActive/BecomeActive exist; session switching is not proof of screen locking. No public lock API was verified locally; report lock capability unavailable until validated. | Existing CoreGraphics combined-session idle; Accessibility permission affects window title, not validity of separately acquired idle. | No local macOS runtime or documentation for history retention and privileges; explicitly unsupported reconstruction rather than guessing. Sleep notification does not prove lid closure or screen-off. |

## Verified sources

1. [Installed logind API manual](/usr/share/man/man5/org.freedesktop.login1.5.gz), sections Manager Signals/Properties, Security, Session Methods/Signals/Properties. It specifies PrepareForSleep/Shutdown before and after operations; delay inhibitors for saving; PreparingFor properties do **not** emit PropertiesChanged; SetLockedHint is desktop supplied. Metadata signal added in systemd 255. [Manager XML](/usr/share/dbus-1/interfaces/org.freedesktop.login1.Manager.xml) and [Session XML](/usr/share/dbus-1/interfaces/org.freedesktop.login1.Session.xml) establish signatures. D-Bus policy: [/usr/share/dbus-1/system.d/org.freedesktop.login1.conf](/usr/share/dbus-1/system.d/org.freedesktop.login1.conf).
2. [journalctl manual](/usr/share/man/man1/journalctl.1.gz): queries show only journal accessible to calling user. [journald configuration](/usr/share/man/man5/journald.conf.5.gz): Storage volatile/persistent/auto/none; size limits may rotate history. [Journal fields](/usr/share/man/man7/systemd.journal-fields.7.gz): _BOOT_ID identifies a boot; __REALTIME_TIMESTAMP is receipt UTC microseconds, __MONOTONIC_TIMESTAMP only meaningful together with boot ID. First/last journal message is not necessarily boot/shutdown boundary.
3. Microsoft generated bindings installed under `/home/userreallyfun/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/windows-sys-0.59.0/src/Windows/Win32/`: `System/Power/mod.rs` line 80 (PowerRegisterSuspendResumeNotification), `UI/WindowsAndMessaging/mod.rs` lines 1568–1574 and 2409/2548/2552 (power/session messages), `System/RemoteDesktop/mod.rs` lines 45–46 and 473/1134 (WTS registration, info class and structure). These are Microsoft API bindings; constants alone do not document message ordering or permissions.
4. Apple generated bindings: `/home/userreallyfun/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/objc2-app-kit-0.3.2/src/generated/NSWorkspace.rs` lines 714–744. Embedded primary-source links: [will sleep](https://developer.apple.com/documentation/appkit/nsworkspacewillsleepnotification?language=objc), [did wake](https://developer.apple.com/documentation/appkit/nsworkspacedidwakenotification?language=objc), [will power off](https://developer.apple.com/documentation/appkit/nsworkspacewillpoweroffnotification?language=objc), [session resign active](https://developer.apple.com/documentation/appkit/nsworkspacesessiondidresignactivenotification?language=objc). Links were not fetched.
5. Existing project native idle/window implementations and their limitations are recorded in `docs/decisions.md`, M2 and M3 Wayland decisions. This research reuses those established implementations rather than asserting new idle evidence.

## Smallest trustworthy implementation

Keep power/session evidence behind a trait independent of window and idle watchers. A timestamped lifecycle event carries source and optional reason. Use current OS state when sampled; use an explicit historical transition only when actually received or verified from retained OS history. Maintain activity and recording health separately: a known awake/AFK interval may still have an Unobserved window backend diagnostic.

Linux: subscribe to logind signals and own-session property changes before reading initial properties; keep timestamps at receipt in UTC milliseconds. Cache state and drain notifications from ordinary sampling. Reading PreparingForSleep once per second is insufficient because both transitions can happen between polls. Acquire a bounded delay inhibitor for sleep/shutdown while flushing live tails; release immediately after durable write. Reacquire for later transitions. Failure to acquire must be visible as degraded durability, never silently claimed reliable. Handle D-Bus disconnect and session disappearance as unknown state plus diagnostic.

On pre-suspend/lock/pause/shutdown, close application/work at the evidence boundary and flush buffered tails immediately. Wake/unlock resumes sampling rather than extending the old focused window. Do not count locked time as active merely because idle was low. A focus error does not erase independently valid AFK/awake evidence. Avoid asking for titles when known locked/asleep.

Use half-open intervals [start,end). Preparation-to-resume intervals are OS lifecycle evidence windows with receipt-time uncertainty: they include preparation/wake scheduling overhead, not exact hardware sleep milliseconds. A canceled preparation must terminate the pending state when false is received. Never label shutdown preparation as confirmed off while the same boot/process remains running. A persisted preparation plus a changed boot identity establishes an intervening lifecycle change, but does not prove the whole gap was physically off. With no validated retained boundary, show unknown recording gap. A changed boot ID alone proves reboot, not power loss or duration. UI text should expose this uncertainty.

Retain explicit pause and orderly tracker-stop markers; tracker exit is not computer shutdown. On restart read evidence first, preserve unknown gaps after crashes and distinguish them from expected-collection backend failures. Existing events JSON can carry additional evidence; if adding persistent lifecycle tables, add a new migration.

## Manual acceptance checks

Linux X11 and each supported Wayland desktop: verify own session resolution; lock/unlock and fast-user-switch separately; suspend/resume with open and hidden window; canceled sleep; sleep while paused; shutdown followed by boot; D-Bus loss; unavailable logind; revoked inhibitor; reboot with journal unreadable/volatile. Check flush persists before sleep and no application duration crosses the interval. Compare source receipt timestamps with journal, allowing documented preparation overhead.

Windows: standard user, locked workstation, RDP reconnect/disconnect, sleep/hibernate/Modern Standby, canceled shutdown, logout vs shutdown, Fast Startup, real reboot, force kill and power interruption. Validate notification thread message loop and querying WTS initial flags. macOS: main-thread notifications with hidden window, sleep/wake, fast-user-switch vs lock, logout/poweroff cancellation, denied Accessibility, restart without retained history. Neither platform was available for these checks.

Skipped: network research and speculative event-log parsers. Risk: native notification boundaries describe OS preparation/observation, not exact physical power state; unsupported history remains an unknown recording gap.

## Implemented scope

- Linux: system-bus logind subscription plus a delay inhibitor for the synchronous
  persistence callback. Inhibitor denial is reported as best-effort flushing.
  Own graphical session is selected by PID or an XDG_SESSION_ID whose User UID is
  checked against the process effective UID. No other user's session is selected.
- Windows: hidden top-level notification window, own-session WTS subscription and
  initial WTSInfoEx lock snapshot; suspend/resume notification registration.
  This implementation's Windows 0.61.2 API types were checked locally in a small
  metadata-only harness; this is not a Windows build or runtime check.
- macOS: retained NSWorkspace observers on the main thread, with block2 (already
  present in the dependency graph). Sleep/wake and shutdown intent only. A local
  type-check attempt could not resolve the uncached objc2-proc-macros package;
  no network download was attempted. Native CI must compile this path.
- No platform currently reconstructs confirmed physical-off intervals or historical
  sleep from OS logs. Access/retention and event interpretation were not verified
  sufficiently here. Off is a representable timeline state for verified evidence;
  none of these intent notifications manufacture it. A restart after an unpaired
  sleep/shutdown signal produces an unknown recording gap.
- Existing durable event JSON stores computer dimensions/source/reason. No schema
  change or migration is necessary. New code uses the computer bucket; pre-upgrade
  AFK data is used only before that bucket's first timestamp.
- Sleep/lock spans are notification receipt intervals, including OS preparation
  and callback scheduling overhead. If a callback timestamp precedes an already
  committed observation, its boundary is clamped to the last computer end to
  preserve monotonic, non-overlapping storage. Unknown sampling gaps are explicit.
- A confirmed locked/suspended state prevents focus/title queries. A missing title
  with valid idle leaves Computer active/AFK; the Applications/Documents tracks
  carry Unobserved and its reason. No recording is distinct from Unobserved.
