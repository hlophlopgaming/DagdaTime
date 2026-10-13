# Wayland setup and manual verification

DagdaTime uses only local compositor sockets and the user's session D-Bus.
Start the daemon (`tracker serve`) or the desktop app before enabling bridges.
The status API reports missing extensions, stale bridge samples and absent idle
protocols explicitly; it never invents successful window tracking.

## GNOME Shell 45–50

Copy `watchers/wayland/gnome` to
`~/.local/share/gnome-shell/extensions/dagdatime@dagdatime.org`, then log out/in
and enable `gnome-extensions enable dagdatime@dagdatime.org`.
The extension sends a focused-window and exact Mutter idle sample once a second.
Disable/uninstall the extension to stop sending samples.

## KDE Plasma 6

Install `watchers/wayland/kde` with
`kpackagetool6 --type KWin/Script --install watchers/wayland/kde` and enable
DagdaTime Watcher in System Settings → Window Management → KWin Scripts.
The QML script sends focused-window samples once a second. Idle time comes
from the compositor's `ext-idle-notify-v1` protocol, independently of the script.

## Sway / Hyprland

Launch DagdaTime in the same desktop session, preserving `SWAYSOCK` or
`HYPRLAND_INSTANCE_SIGNATURE` and `XDG_RUNTIME_DIR`. No extension is needed.
The daemon queries Sway's GET_TREE or Hyprland's JSON activewindow over Unix
sockets; idle time uses `ext-idle-notify-v1`.

The idle protocol measures elapsed idle after the watcher starts; it cannot
recover input history before startup. Version 2 ignores idle inhibitors; on
version 1 applications inhibiting idle can suppress idle notifications.

## Verify on each compositor

Run `tracker debug`, switch between native apps and XWayland apps, edit a
Unicode window title, leave input idle, then move the mouse. Confirm identity,
title and PID update, idle grows and resets. Start the desktop app and confirm
its Settings status matches. Pause and confirm the SQLite intervals stop growing;
resume must start a separate interval. Disable the bridge / remove access to IPC
and confirm an explicit error appears. Re-enable it and confirm recovery.

GNOME and KDE bridges need desktop testing; CI tests the IPC payload parsers
and bridge freshness without installing anything into a user's session.

References: [KWin QML D-Bus](https://api.kde.org/qml-org-kde-kwin-dbuscall.html),
[KWin packages](https://develop.kde.org/docs/plasma/kwin/),
[GNOME extensions](https://gjs.guide/extensions/development/creating.html),
[idle protocol](https://wayland.app/protocols/ext-idle-notify-v1).
