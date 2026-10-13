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

Settings can select the next-launch data folder and enable **Start Draugr Time
when I log in**. Startup launches with `--autostart --minimized`. After moving
or renaming the executable, toggle startup again to register its current path.

Linux uses GTK/AppIndicator for the tray and supports X11 and Wayland windows.
Windows and macOS use native tray and icon providers. Real focus, idle,
permissions, tray actions, folder selection and startup registration need
manual verification on each OS; CI checks builds and diagnostic launches.
