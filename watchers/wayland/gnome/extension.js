import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class DraugrTimeExtension extends Extension {
    _call(method, signature, values, complete = null) {
        Gio.DBus.session.call('org.dagdatime.Watcher', '/org/dagdatime/Watcher',
            'org.dagdatime.Watcher', method, new GLib.Variant(signature, values),
            null, Gio.DBusCallFlags.NONE, 500, null, (connection, result) => {
                try { const reply = connection.call_finish(result); complete?.(reply); }
                catch (_) { /* daemon may be closed */ }
            });
    }
    _publish() {
        const window = this._window;
        const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
        this._call('UpdateWindow', '(ssss)', ['gnome', app?.get_id() ?? '',
            window?.get_title() ?? '', String(Math.max(0, window?.get_pid() ?? 0))]);
    }
    _focus() {
        if (this._window && this._titleSignal)
            this._window.disconnect(this._titleSignal);
        this._window = global.display.focus_window;
        this._titleSignal = this._window?.connect('notify::title', () => this._publish());
        this._publish();
    }
    enable() {
        this._enabled = true;
        this._focusSignal = global.display.connect('notify::focus-window', () => this._focus());
        this._focus();
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
            const idle = String(Math.max(0, global.backend.get_core_idle_monitor().get_idletime()));
            this._call('Heartbeat', '(ss)', ['gnome', idle], reply => {
                // A new daemon has no window yet; resynchronise without polling titles.
                if (this._enabled && !reply.deep_unpack()[0]) this._publish();
            });
            return GLib.SOURCE_CONTINUE;
        });
    }
    disable() {
        this._enabled = false;
        if (this._timer) GLib.source_remove(this._timer);
        if (this._focusSignal) global.display.disconnect(this._focusSignal);
        if (this._window && this._titleSignal) this._window.disconnect(this._titleSignal);
        this._timer = this._focusSignal = this._titleSignal = null;
        this._window = null;
    }
}
