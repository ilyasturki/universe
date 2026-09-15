from PySide6.QtCore import Property, Signal, Slot

from .settings import RowsForm, _add, _dig, _row, auto_rate, launch_row, proton_choices, screen_label

SECTIONS = ["Gamescope", "Overlay and cursor", "Proton"]


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
        keys = self._client.launchKeys("global", mode)
        protons = proton_choices(config)
        hz = auto_rate(mode, launch.get("gamescope", True), launch.get("gamescope_refresh"))
        rows, groups = [], []
        for section in SECTIONS:
            for spec in keys:
                if spec["section"] != section:
                    continue
                value = launch.get(spec["key"])
                if value in (None, ""):
                    value = spec["default"]
                _add(rows, groups, section, launch_row("Launch", spec, value, protons=protons, auto_hz=hz), caps=True)
            if section == "Overlay and cursor":
                _add(rows, groups, section, _row("Launch", "desktop.hide_cursor", "Hide the cursor while playing", "bool", bool(_dig(config, "desktop.hide_cursor", True))), caps=True)
        groups[0]["meta"] = self._screen
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        return self._client.setConfig(row["key"], payload)

    def _reload(self, row):
        self.load()

    screen = Property(str, lambda self: self._screen, notify=screenChanged)
