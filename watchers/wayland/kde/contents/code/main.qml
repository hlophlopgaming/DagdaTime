import QtQuick
import org.kde.kwin 3.0

Item {
    DBusCall {
        id: sender
        service: "org.dagdatime.Watcher"
        path: "/org/dagdatime/Watcher"
        dbusInterface: "org.dagdatime.Watcher"
        method: "UpdateWindow"
    }
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            const window = Workspace.activeWindow;
            sender.arguments = ["kde", window ? String(window.resourceClass) : "",
                window ? String(window.caption) : "", window ? String(Math.max(0, window.pid)) : "0"];
            sender.call();
        }
    }
}
