from PySide6.QtCore import Property, Signal, Slot

from .settings import HIDE_CURSOR, RowsForm, _add, _dig, _row, global_launch_rows, screen_label


class LaunchForm(RowsForm):
    screenChanged = Signal()

    def __init__(self, client, screen_mode=lambda: {}, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._screen = ""

    @Slot()
    def load(self):
        config = self._client.config()
        mode = self._screen_mode()
        self._screen = " ".join(p for p in (str(mode.get("screen") or ""), screen_label(mode)) if p)
        self.screenChanged.emit()
        rows, groups = [], []
        # A key tied to a runner is set on that runner's page.
        global_launch_rows(rows, groups, self._client, config, mode, lambda spec: not spec["runners"])
        _add(rows, groups, "Overlay", _row("Overlay", "desktop.hide_cursor", "Hide the cursor while playing", "bool", bool(_dig(config, "desktop.hide_cursor", True)), detail=HIDE_CURSOR), caps=True)
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        return self._client.setConfig(row["key"], payload)

    def _reload(self, row):
        self.load()

    screen = Property(str, lambda self: self._screen, notify=screenChanged)
