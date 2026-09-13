import Gio from 'gi://Gio';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

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
  </interface>
</node>`;

export default class UniverseExtension extends Extension {
    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        this._nameId = Gio.bus_own_name(
            Gio.BusType.SESSION, BUS_NAME, Gio.BusNameOwnerFlags.NONE,
            null, null, null);
    }

    disable() {
        if (this._nameId) {
            Gio.bus_unown_name(this._nameId);
            this._nameId = 0;
        }
        if (this._dbus) {
            this._dbus.unexport();
            this._dbus = null;
        }
    }

    // The ids are what org.gnome.Mutter.ScreenCast.Session.RecordWindow takes as window-id.
    List() {
        const focus = global.display.get_focus_window();
        const windows = [];
        for (const actor of global.get_window_actors()) {
            const w = actor.get_meta_window();
            if (!w || w.is_override_redirect())
                continue;
            const r = w.get_frame_rect();
            const ws = w.get_workspace();
            windows.push({
                id: w.get_id(),
                pid: w.get_pid(),
                wm_class: w.get_wm_class(),
                title: w.get_title(),
                focused: w === focus,
                width: r.width,
                height: r.height,
                workspace: ws ? ws.index() : -1,
                hidden: w.is_hidden(),
                minimized: w.minimized,
            });
        }
        return JSON.stringify(windows);
    }

    // The shell's own media-key OSD, on every monitor; org.gnome.Shell.ShowOSD refuses callers other
    // than gsd. A negative level draws no bar, an empty label none.
    ShowOSD(icon, label, level) {
        Main.osdWindowManager.show(-1, Gio.ThemedIcon.new(icon), label || null, level < 0 ? null : level);
    }
}
