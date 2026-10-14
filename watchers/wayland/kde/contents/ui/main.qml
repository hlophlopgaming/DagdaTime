import QtQuick
import org.kde.kwin 3.0

Item {
    id: root
    function publish() {
        const window = Workspace.activeWindow;
        sender.arguments = ["kde", window ? String(window.resourceClass) : "",
            window ? String(window.caption) : "", window ? String(Math.max(0, window.pid)) : "0"];
        sender.call();
    }
    DBusCall {
        id: sender
        service: "org.dagdatime.Watcher"
        path: "/org/dagdatime/Watcher"
        dbusInterface: "org.dagdatime.Watcher"
        method: "UpdateWindow"
    }
    DBusCall {
        id: heartbeat
        service: "org.dagdatime.Watcher"
        path: "/org/dagdatime/Watcher"
        dbusInterface: "org.dagdatime.Watcher"
        method: "Heartbeat"
        arguments: ["kde", ""]
    }
    Connections {
        target: Workspace
        function onWindowActivated() { root.publish(); }
    }
    Connections {
        target: Workspace.activeWindow
        function onCaptionChanged() { root.publish(); }
    }
    Component.onCompleted: publish()
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: heartbeat.call()
    }
    // Refresh after daemon restarts, even if focus and title have not changed.
    Timer {
        interval: 30000
        running: true
        repeat: true
        onTriggered: root.publish()
    }
}
