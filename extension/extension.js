import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.universe.Windows';
const OBJECT_PATH = '/org/universe/Windows';

const IFACE = `
<node>
  <interface name="org.universe.Windows">
    <method name="List">
      <arg type="s" name="windows" direction="out"/>
    </method>
    <method name="ShowOSD">
      <arg type="s" name="icon" direction="in"/>
      <arg type="s" name="label" direction="in"/>
      <arg type="d" name="level" direction="in"/>
    </method>
    <method name="Activate">
      <arg type="t" name="id" direction="in"/>
      <arg type="b" name="ok" direction="out"/>
    </method>
    <method name="Screenshot">
      <arg type="s" name="path" direction="in"/>
      <arg type="b" name="window" direction="in"/>
      <arg type="b" name="cursor" direction="in"/>
      <arg type="b" name="ok" direction="out"/>
    </method>
  </interface>
</node>`;

export default class UniverseExtension extends Extension {
    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        this._nameId = Gio.bus_own_name(Gio.BusType.SESSION, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null, null);
    }

    disable() {
        Gio.bus_unown_name(this._nameId);
        this._dbus.unexport();
        this._dbus = null;
    }

    // The ids are what org.gnome.Mutter.ScreenCast.Session.RecordWindow takes as window-id.
    List() {
        const focus = global.display.get_focus_window();
        const windows = [];
        for (const actor of global.get_window_actors()) {
            const w = actor.get_meta_window();
            if (!w || w.is_override_redirect()) continue;
            const r = w.get_frame_rect();
            windows.push({
                id: w.get_id(),
                pid: w.get_pid(),
                wm_class: w.get_wm_class(),
                title: w.get_title(),
                focused: w === focus,
                width: r.width,
                height: r.height,
                hidden: w.is_hidden(),
                minimized: w.minimized,
            });
        }
        return JSON.stringify(windows);
    }

    Activate(id) {
        for (const actor of global.get_window_actors()) {
            const w = actor.get_meta_window();
            if (!w || w.get_id() !== Number(id)) continue;
            w.activate(global.get_current_time());
            return true;
        }
        return false;
    }

    // org.gnome.Shell.Screenshot refuses background callers; screenshot*() grabs the pixels before its async PNG encode.
    ScreenshotAsync([path, window, cursor], invocation) {
        const reply = (ok) => invocation.return_value(new GLib.Variant('(b)', [ok]));
        let stream;
        try {
            const file = Gio.File.new_for_path(path);
            const parent = file.get_parent();
            if (parent) GLib.mkdir_with_parents(parent.get_path(), 0o755);
            stream = file.replace(null, false, Gio.FileCreateFlags.REPLACE_DESTINATION, null);
        } catch (e) {
            logError(e, 'Universe: cannot open the screenshot file');
            reply(false);
            return;
        }
        const area = window ? this._focusedWindowRect() : this._primaryMonitorRect();
        if (!area) {
            try {
                stream.close(null);
            } catch {}
            reply(false);
            return;
        }
        try {
            const shooter = new Shell.Screenshot();
            const finish = (_o, res) => {
                try {
                    if (window) shooter.screenshot_window_finish(res);
                    else shooter.screenshot_finish(res);
                } catch (e) {
                    logError(e, 'Universe: screenshot failed');
                    Main.notifyError('Universe', 'Screenshot failed to save');
                }
                try {
                    stream.close(null);
                } catch {}
            };
            if (window) shooter.screenshot_window(false, cursor, stream, finish);
            else shooter.screenshot(cursor, stream, finish);
        } catch (e) {
            logError(e, 'Universe: screenshot failed');
            try {
                stream.close(null);
            } catch {}
            reply(false);
            return;
        }
        reply(true);
    }

    _focusedWindowRect() {
        const win = global.display.focus_window;
        if (!win) return null;
        const r = win.get_frame_rect();
        return { x: r.x, y: r.y, width: r.width, height: r.height };
    }

    _primaryMonitorRect() {
        const m = Main.layoutManager.primaryMonitor;
        return m ? { x: m.x, y: m.y, width: m.width, height: m.height } : null;
    }

    // org.gnome.Shell.ShowOSD refuses callers other than gsd; a negative level draws no bar; Shell 50 renamed show(-1, …) showAll.
    ShowOSD(icon, label, level) {
        const manager = Main.osdWindowManager;
        const gicon = Gio.ThemedIcon.new(icon);
        const bar = level < 0 ? null : level;
        if (manager.showAll) manager.showAll(gicon, label || null, bar, 1);
        else manager.show(-1, gicon, label || null, bar, 1);
    }
}
