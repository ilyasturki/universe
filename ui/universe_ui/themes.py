from PySide6.QtCore import QObject, Signal, Slot

from .qt import Property

THEMES = [
    {
        "id": "reprise",
        "name": "Reprise",
        "entry": "theme.qml",
        "overlay": "ui/Dock.qml",
        "frame": True,
        "ground": "#0e0f13",
        "detail": "Dark, cinematic: the game's art behind everything.",
    },
    {
        "id": "switch2",
        "name": "Switch 2",
        "entry": "switch2/theme.qml",
        "overlay": "",
        "frame": False,
        "ground": "#ebebeb",
        "detail": "The Switch 2 HOME menu.",
    },
    {
        "id": "ps5",
        "name": "PS5",
        "entry": "ps5/theme.qml",
        "overlay": "",
        "frame": False,
        "ground": "#0b0d12",
        "detail": "The PS5 home screen: the game's world behind a row of tiles.",
    },
]
DEFAULT = "reprise"
MEMORY_KEY = "theme"


def theme_by_id(ident):
    return next((t for t in THEMES if t["id"] == ident), None)


class ThemeSelector(QObject):
    changed = Signal()
    fontChanged = Signal()

    def __init__(self, memory, initial="", parent=None):
        super().__init__(parent)
        self._memory = memory
        self._current = theme_by_id(initial) or theme_by_id(memory.get(MEMORY_KEY)) or theme_by_id(DEFAULT) or THEMES[0]
        self._landing = ""

    # A switch rebuilds the whole tree: the new look opens on its Themes page, once.
    @Slot(str, result=bool)
    def set(self, ident):
        theme = theme_by_id(ident)
        if theme is None:
            return False
        self._memory.set(MEMORY_KEY, theme["id"])
        if theme is not self._current:
            self._current = theme
            self._landing = "themes"
            self.changed.emit()
            self.fontChanged.emit()
        return True

    @Slot(result=str)
    def takeLanding(self):
        landing, self._landing = self._landing, ""
        return landing

    # Stored in ui-memory.json: renaming a look's id orphans its font.
    def _font_key(self):
        return self._current["id"] + "Font"

    def _font(self):
        return self._memory.get(self._font_key()) or ""

    def _set_font(self, path):
        if path == self._font():
            return
        if path:
            self._memory.set(self._font_key(), path)
        else:
            self._memory.unset(self._font_key())
        self.fontChanged.emit()

    themes = Property(list, lambda self: [dict(t) for t in THEMES], constant=True)
    current = Property(str, lambda self: self._current["id"], notify=changed)
    landing = Property(str, lambda self: self._landing, notify=changed)
    name = Property(str, lambda self: self._current["name"], notify=changed)
    entry = Property(str, lambda self: self._current["entry"], notify=changed)
    overlay = Property(str, lambda self: self._current["overlay"], notify=changed)
    # Whether the look bridges the swap with the game's last frame; without it HOME need not wait for one.
    frame = Property(bool, lambda self: bool(self._current["frame"]), notify=changed)
    ground = Property(str, lambda self: self._current["ground"], notify=changed)
    fontPath = Property(str, _font, _set_font, notify=fontChanged)
