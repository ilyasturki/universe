from PySide6.QtCore import Property, QObject, Signal, Slot

THEMES = [
    {"id": "reprise", "name": "Reprise", "entry": "theme.qml", "overlay": "ui/Dock.qml", "ground": "#0e0f13",
     "detail": "Dark, cinematic: the game's art behind everything."},
    {"id": "switch2", "name": "Switch 2", "entry": "switch2/theme.qml", "overlay": "", "ground": "#ebebeb",
     "detail": "The Switch 2 HOME menu."},
]
DEFAULT = "reprise"
# ui-memory.json written before the black variant went away
LEGACY_IDS = {"switch2-white": "switch2", "switch2-black": "switch2"}
MEMORY_KEY = "theme"
FONT_KEY = "switch2Font"


def theme_by_id(ident):
    ident = LEGACY_IDS.get(ident, ident)
    return next((t for t in THEMES if t["id"] == ident), None)


class ThemeSelector(QObject):
    changed = Signal()
    fontChanged = Signal()

    def __init__(self, memory, initial="", parent=None):
        super().__init__(parent)
        self._memory = memory
        self._current = theme_by_id(initial) or theme_by_id(memory.get(MEMORY_KEY)) or theme_by_id(DEFAULT)

    @Slot(str, result=bool)
    def set(self, ident):
        theme = theme_by_id(ident)
        if theme is None:
            return False
        self._memory.set(MEMORY_KEY, theme["id"])
        if theme is not self._current:
            self._current = theme
            self.changed.emit()
        return True

    def _font(self):
        return self._memory.get(FONT_KEY) or ""

    def _set_font(self, path):
        if path == self._font():
            return
        if path:
            self._memory.set(FONT_KEY, path)
        else:
            self._memory.unset(FONT_KEY)
        self.fontChanged.emit()

    themes = Property("QVariantList", lambda self: [dict(t) for t in THEMES], constant=True)
    current = Property(str, lambda self: self._current["id"], notify=changed)
    name = Property(str, lambda self: self._current["name"], notify=changed)
    entry = Property(str, lambda self: self._current["entry"], notify=changed)
    overlay = Property(str, lambda self: self._current["overlay"], notify=changed)
    ground = Property(str, lambda self: self._current["ground"], notify=changed)
    fontPath = Property(str, _font, _set_font, notify=fontChanged)
