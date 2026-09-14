"""Settings › Launch: what every game starts with — gamescope and its fields against the screen
the launcher is on, the overlay and the cursor, the Proton defaults. Every row is a config.toml key."""

from PySide6.QtCore import Property, Signal, Slot

from .settings import RowsForm, _dig, _group, _row, _to_bus, choice_row, gamescope_rows, screen_label

PROTON_ROWS = [
    ("launch.esync", "Esync", "bool"), ("launch.fsync", "Fsync", "bool"), ("launch.ntsync", "NTSync", "bool"),
    ("launch.wayland", "Wayland", "bool"), ("launch.hdr", "HDR", "bool"), ("launch.dlss_upgrade", "DLSS upgrade", "bool"),
    ("launch.fsr4_upgrade", "FSR 4 upgrade", "bool"), ("launch.xess_upgrade", "XeSS upgrade", "bool"), ("launch.optiscaler", "OptiScaler", "bool"),
]


class LaunchForm(RowsForm):
    screenChanged = Signal()

    def __init__(self, client, screen_name=lambda: "", parent=None):
        super().__init__(client, parent)
        self._screen_name = screen_name
        self._screen = ""

    @Slot()
    def load(self):
        config = self._client.config() or {}
        launch = config.get("launch") or {}
        mode = self._client.screenMode(self._screen_name()) or {}
        self._screen = " ".join(p for p in (str(mode.get("screen") or ""), screen_label(mode)) if p)
        self.screenChanged.emit()
        rows, groups = [], []

        def add(group, row):
            if not groups or groups[-1]["title"] != group:
                groups.append(_group(group, [], caps=True))
            groups[-1]["rows"].append(len(rows))
            rows.append(row)

        add("Gamescope", _row("Launch", "launch.gamescope", "Gamescope", "bool", bool(launch.get("gamescope", True))))
        for key, label, kind, choices, values in gamescope_rows(mode):
            value = launch.get(key.split(".", 1)[1])
            if kind == "bool":
                value = bool(value)
            add("Gamescope", choice_row("Launch", key, label, kind, value, choices, values))
        add("Gamescope", _row("Launch", "launch.gamescope_args", "Arguments", "string", launch.get("gamescope_args") or ""))
        groups[0]["meta"] = self._screen
        add("Overlay and cursor", _row("Launch", "launch.mangohud", "MangoHud", "bool", bool(launch.get("mangohud", True))))
        add("Overlay and cursor", _row("Launch", "desktop.hide_cursor", "Hide the cursor while playing", "bool", bool(_dig(config, "desktop.hide_cursor", True))))
        choices = sorted((config.get("proton") or {}).keys())
        default = str(launch.get("proton") or "")
        if default and default not in choices:
            choices.insert(0, default)
        add("Proton", _row("Launch", "launch.proton", "Proton", "enum", default, choices))
        for key, label, kind in PROTON_ROWS:
            add("Proton", _row("Launch", key, label, kind, bool(launch.get(key.split(".", 1)[1], False))))
        self._set_rows(rows, groups)

    @Slot(int, "QVariant", result=bool)
    def setValue(self, index, value):
        if not (0 <= index < len(self._rows)):
            return False
        row = self._rows[index]
        payload = _to_bus(row["type"], value)
        if row.get("choiceValues") and payload in row["choices"]:
            payload = row["choiceValues"][row["choices"].index(payload)]
        ok = self._client.setConfig(row["key"], payload)
        if ok:
            self.load()
        return bool(ok)

    @Slot(int)
    def toggle(self, index):
        row = self.row(index)
        if row.get("type") == "bool":
            self.setValue(index, not row.get("value"))

    screen = Property(str, lambda self: self._screen, notify=screenChanged)
