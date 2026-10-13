import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class DagdaTimeExtension extends Extension {
    enable() {
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
            const window = global.display.focus_window;
            const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
            const values = ['gnome', app?.get_id() ?? '', window?.get_title() ?? '',
                String(Math.max(0, window?.get_pid() ?? 0)),
                String(Math.max(0, global.backend.get_core_idle_monitor().get_idletime()))];
            Gio.DBus.session.call('org.dagdatime.Watcher', '/org/dagdatime/Watcher',
                'org.dagdatime.Watcher', 'Update', new GLib.Variant('(sssss)', values),
                null, Gio.DBusCallFlags.NONE, 500, null, (connection, result) => {
                    try { connection.call_finish(result); } catch (_) { /* daemon may be closed */ }
                });
            return GLib.SOURCE_CONTINUE;
        });
    }
    disable() { if (this._timer) GLib.source_remove(this._timer); this._timer = null; }
}
