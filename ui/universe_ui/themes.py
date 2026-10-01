import os

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
        "unlocked": "ACHIEVEMENT UNLOCKED",
    },
    {
        "id": "switch2",
        "name": "Switch 2",
        "entry": "switch2/theme.qml",
        "overlay": "",
        "frame": False,
        "ground": "#ebebeb",
        "detail": "The Switch 2 HOME menu.",
        "unlocked": "ACHIEVEMENT UNLOCKED",
    },
    {
        "id": "ps5",
        "name": "PS5",
        "entry": "ps5/theme.qml",
        "overlay": "ps5/ui/ControlCenter.qml",
        "frame": False,
        "ground": "#0b0d12",
        "detail": "The PS5 home screen: the game's world behind a row of tiles.",
        "unlocked": "TROPHY EARNED",
    },
]
AFFILIATION = "Not affiliated with Nintendo or Sony"
TRADEMARKS = "Nintendo Switch is a trademark of Nintendo; PlayStation and PS5 are trademarks of Sony Interactive Entertainment."
DEFAULT = "reprise"
MEMORY_KEY = "theme"
BOOT_KEY = "bootIntro"


def theme_by_id(ident):
    return next((t for t in THEMES if t["id"] == ident), None)


class ThemeSelector(QObject):
    changed = Signal()
    fontChanged = Signal()
    soundsChanged = Signal()
    bootChanged = Signal()

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
            self.soundsChanged.emit()
        return True

    @Slot(result=str)
    def takeLanding(self):
        landing, self._landing = self._landing, ""
        return landing

    # Stored in ui-memory.json as <id>Font and <id>Sounds: renaming a look's id orphans them.
    def _own(self, suffix):
        return self._memory.get(self._current["id"] + suffix) or ""

    def _set_own(self, suffix, value, signal):
        if value == self._own(suffix):
            return
        if value:
            self._memory.set(self._current["id"] + suffix, value)
        else:
            self._memory.unset(self._current["id"] + suffix)
        signal.emit()

    def _font(self):
        return self._own("Font")

    def _set_font(self, path):
        self._set_own("Font", path, self.fontChanged)

    def _sounds(self):
        return self._own("Sounds")

    def _set_sounds(self, path):
        self._set_own("Sounds", path, self.soundsChanged)

    def _sound_files(self):
        folder = self._sounds()
        try:
            names = sorted(os.listdir(folder)) if folder else []
        except OSError:
            return {}
        return {n[:-4].lower(): "file://" + os.path.join(folder, n) for n in names if n.lower().endswith(".wav")}

    def _boot(self):
        return self._memory.get(BOOT_KEY) is not False

    def _set_boot(self, on):
        if bool(on) != self._boot():
            self._memory.set(BOOT_KEY, bool(on))
            self.bootChanged.emit()

    themes = Property(list, lambda self: [dict(t) for t in THEMES], constant=True)
    affiliation = Property(str, lambda self: AFFILIATION, constant=True)
    trademarks = Property(str, lambda self: TRADEMARKS, constant=True)
    current = Property(str, lambda self: self._current["id"], notify=changed)
    landing = Property(str, lambda self: self._landing, notify=changed)
    name = Property(str, lambda self: self._current["name"], notify=changed)
    entry = Property(str, lambda self: self._current["entry"], notify=changed)
    overlay = Property(str, lambda self: self._current["overlay"], notify=changed)
    # Whether the look bridges the swap with the game's last frame; without it HOME need not wait for one.
    frame = Property(bool, lambda self: bool(self._current["frame"]), notify=changed)
    ground = Property(str, lambda self: self._current["ground"], notify=changed)
    # The heading of the overlay's card for an achievement unlocked mid-game, in the look's own words.
    unlocked = Property(str, lambda self: self._current["unlocked"], notify=changed)
    fontPath = Property(str, _font, _set_font, notify=fontChanged)
    # A folder of WAVs named as the look's sounds (ok.wav, tick.wav…): each one there replaces the bundled one.
    soundsPath = Property(str, _sounds, _set_sounds, notify=soundsChanged)
    # {name: file URL} of the WAVs in soundsPath
    soundFiles = Property("QVariantMap", _sound_files, notify=soundsChanged)
    # The startup animation, every look's: on until ui-memory.json's bootIntro says false.
    bootIntro = Property(bool, _boot, _set_boot, notify=bootChanged)
