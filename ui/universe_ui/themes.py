import logging
import os

from PySide6.QtCore import QObject, QUrl, Signal, Slot

from .qt import Property

THEMES = [
    {
        "id": "reprise",
        "name": "Reprise",
        "entry": "theme.qml",
        "overlay": "ui/Dock.qml",
        "osd": "ui/VolumeOsd.qml",
        "frame": True,
        "ground": "#0e0f13",
        "accent": "#5aa0ff",
        "detail": "Dark, cinematic: the game's art behind everything.",
        "unlocked": "ACHIEVEMENT UNLOCKED",
    },
    {
        "id": "switch2",
        "name": "Switch 2",
        "entry": "switch2/theme.qml",
        "overlay": "",
        "osd": "ui/VolumePill.qml",
        "frame": False,
        "ground": "#ebebeb",
        "accent": "#1656b9",
        "detail": "The Switch 2 HOME menu.",
        "unlocked": "ACHIEVEMENT UNLOCKED",
    },
    {
        "id": "ps5",
        "name": "PS5",
        "entry": "ps5/theme.qml",
        "overlay": "ps5/ui/ControlCenter.qml",
        "osd": "ui/VolumePill.qml",
        "frame": False,
        "ground": "#0b0d12",
        "accent": "#3b8ff0",
        "detail": "The PS5 home screen: the game's world behind a row of tiles.",
        "unlocked": "TROPHY EARNED",
    },
]
AFFILIATION = "Not affiliated with Nintendo or Sony"
TRADEMARKS = "Nintendo Switch is a trademark of Nintendo; PlayStation and PS5 are trademarks of Sony Interactive Entertainment."
DEFAULT = "reprise"
MEMORY_KEY = "theme"
BOOT_KEY = "bootIntro"
# What an installed theme cannot name: it draws in the main window alone, and the host's own pill shows the volume over a game.
INSTALLED = {"overlay": "", "osd": "ui/VolumePill.qml", "frame": False, "ground": "#000000", "accent": "#ffffff", "unlocked": "ACHIEVEMENT UNLOCKED"}

log = logging.getLogger("universe.themes")


def _url(path):
    return QUrl.fromLocalFile(path).toString() if path else ""


def installed_look(theme):
    """A row of the core's `themes()` as a look; `unavailable` says why it cannot be picked."""
    unavailable = theme.get("incompatible") or ("" if theme.get("entry") else "its theme.qml is missing")
    return {
        **INSTALLED,
        "id": theme["id"],
        "name": theme.get("name") or theme["id"],
        "entry": _url(theme.get("entry")),
        "detail": theme.get("description") or "",
        "screenshot": _url(theme.get("screenshot")),
        "installed": True,
        "unavailable": unavailable,
    }


BUILT_IN = [{**t, "screenshot": "", "installed": False, "unavailable": ""} for t in THEMES]


class ThemeSelector(QObject):
    changed = Signal()
    listChanged = Signal()
    fontChanged = Signal()
    soundsChanged = Signal()
    bootChanged = Signal()

    # installed: the core's `themes()`, read again by rescan().
    def __init__(self, memory, initial="", parent=None, installed=None):
        super().__init__(parent)
        self._memory = memory
        self._installed = installed or list
        self._themes = self._read()
        self._landing = ""
        self._notice = ""
        wanted = self._by_id(initial) or self._by_id(memory.get(MEMORY_KEY))
        if wanted is not None and wanted["unavailable"]:
            self._notice = f"{wanted['name']} can't be used: {wanted['unavailable']}"
            wanted = None
        self._current = wanted or self._by_id(DEFAULT) or self._themes[0]

    def _read(self):
        builtin = {t["id"] for t in BUILT_IN}
        return BUILT_IN + [installed_look(t) for t in self._installed() if t["id"] not in builtin]

    def _by_id(self, ident):
        return next((t for t in self._themes if t["id"] == ident), None)

    def _switch(self, theme, landing):
        self._memory.set(MEMORY_KEY, theme["id"])
        if theme is not self._current:
            self._current = theme
            self._landing = landing
            self.changed.emit()
            self.fontChanged.emit()
            self.soundsChanged.emit()

    # A switch rebuilds the whole tree: the new look opens on its Themes page, once.
    @Slot(str, result=bool)
    def set(self, ident):
        theme = self._by_id(ident)
        if theme is None or theme["unavailable"]:
            return False
        self._switch(theme, "themes")
        return True

    def _fall_back(self, notice):
        default = self._by_id(DEFAULT)
        if default is None or self._current is default:
            return
        self._notice = f"{notice}: back on {default['name']}"
        self._switch(default, "")

    @Slot()
    def rescan(self):
        """The installed themes read again, after an install or a removal; the current one gone, the default look takes over."""
        self._themes = self._read()
        self.listChanged.emit()
        now = self._by_id(self._current["id"])
        if now is None or now["unavailable"]:
            self._fall_back(f"{self._current['name']} is no longer installed" if now is None else f"{now['name']} can't be used")
        else:
            self._current = now

    @Slot()
    def failed(self):
        """main.qml's Loader could not load the current look: QML has logged why."""
        log.warning("%s failed to load", self._current["entry"])
        self._fall_back(f"{self._current['name']} failed to load")

    # What a fall back to the default look says, for the look that takes over to show once.
    @Slot(result=str)
    def takeNotice(self):
        notice, self._notice = self._notice, ""
        return notice

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

    themes = Property(list, lambda self: [dict(t) for t in self._themes], notify=listChanged)
    affiliation = Property(str, lambda self: AFFILIATION, constant=True)
    trademarks = Property(str, lambda self: TRADEMARKS, constant=True)
    current = Property(str, lambda self: self._current["id"], notify=changed)
    landing = Property(str, lambda self: self._landing, notify=changed)
    name = Property(str, lambda self: self._current["name"], notify=changed)
    entry = Property(str, lambda self: self._current["entry"], notify=changed)
    overlay = Property(str, lambda self: self._current["overlay"], notify=changed)
    osd = Property(str, lambda self: self._current["osd"], notify=changed)
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
