from PySide6.QtCore import Property, Signal, Slot

from .settings import RowsForm, _add, _dig, _row, choice_row, fps_limit_row, gamescope_rows, screen_label

PROTON_ROWS = [
    ("launch.esync", "Esync", "bool"), ("launch.fsync", "Fsync", "bool"), ("launch.ntsync", "NTSync", "bool"),
    ("launch.wayland", "Wayland", "bool"), ("launch.hdr", "HDR", "bool"), ("launch.dlss_upgrade", "DLSS upgrade", "bool"),
    ("launch.fsr4_upgrade", "FSR 4 upgrade", "bool"), ("launch.xess_upgrade", "XeSS upgrade", "bool"), ("launch.optiscaler", "OptiScaler", "bool"),
]


class LaunchForm(RowsForm):
    screenChanged = Signal()

    def __init__(self, client, screen_mode=lambda: {}, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._screen = ""

    @Slot()
    def load(self):
        config = self._client.config() or {}
        launch = config.get("launch") or {}
        mode = self._screen_mode() or {}
        self._screen = " ".join(p for p in (str(mode.get("screen") or ""), screen_label(mode)) if p)
        self.screenChanged.emit()
        rows, groups = [], []
        _add(rows, groups, "Gamescope", _row("Launch", "launch.gamescope", "Gamescope", "bool", bool(launch.get("gamescope", True))), caps=True)
        for key, label, kind, choices, values in gamescope_rows(mode):
            value = launch.get(key.split(".", 1)[1])
            if kind == "bool":
                value = bool(value)
            _add(rows, groups, "Gamescope", choice_row("Launch", key, label, kind, value, choices, values), caps=True)
        _add(rows, groups, "Gamescope", _row("Launch", "launch.gamescope_args", "Arguments", "string", launch.get("gamescope_args") or ""), caps=True)
        groups[0]["meta"] = self._screen
        _add(rows, groups, "Overlay and cursor", _row("Launch", "launch.mangohud", "MangoHud", "bool", bool(launch.get("mangohud", True))), caps=True)
        _add(rows, groups, "Overlay and cursor", fps_limit_row("Launch", launch.get("fps_limit"), mode, gamescope=bool(launch.get("gamescope", True)), gamescope_refresh=launch.get("gamescope_refresh")), caps=True)
        _add(rows, groups, "Overlay and cursor", _row("Launch", "desktop.hide_cursor", "Hide the cursor while playing", "bool", bool(_dig(config, "desktop.hide_cursor", True))), caps=True)
        choices = sorted((config.get("proton") or {}).keys())
        default = str(launch.get("proton") or "")
        if default and default not in choices:
            choices.insert(0, default)
        _add(rows, groups, "Proton", _row("Launch", "launch.proton", "Proton", "enum", default, choices), caps=True)
        for key, label, kind in PROTON_ROWS:
            _add(rows, groups, "Proton", _row("Launch", key, label, kind, bool(launch.get(key.split(".", 1)[1], False))), caps=True)
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        return self._client.setConfig(row["key"], payload)

    def _reload(self, row):
        self.load()

    screen = Property(str, lambda self: self._screen, notify=screenChanged)
