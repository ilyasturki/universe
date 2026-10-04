from collections.abc import Callable

from PySide6.QtCore import Signal, Slot

from ..qt import Property
from .settings import RowsForm, launch_cards, screen_label


def build_launch(client, screen_mode):
    """The global Launch page, as the core's launch form lists it: Display and Overlay, then behind the gate the scaling,
    environment, programs, folders, keys, desktop and Proton builds cards."""
    mode = screen_mode()
    screen = " ".join(p for p in (str(mode.get("screen") or ""), screen_label(mode)) if p)
    rows, groups = [], []
    launch_cards(client.form("launch", "", mode), rows, groups, mode, client.gpu())
    return rows, groups, screen


class LaunchForm(RowsForm):
    screenChanged = Signal()

    def __init__(self, client, screen_mode: Callable[[], dict] = dict, parent=None):
        super().__init__(client, parent)
        self._screen_mode = screen_mode
        self._screen = ""

    @Slot()
    def load(self):
        rows, groups, self._screen = build_launch(self._client, self._screen_mode)
        self.screenChanged.emit()
        self._set_rows(rows, groups)

    def _write(self, row, payload):
        return self._client.setField("launch", "", row["field"], payload)

    def _reload(self, row):
        self.load()

    screen = Property(str, lambda self: self._screen, notify=screenChanged)
