import json
import os
from pathlib import Path

from PySide6.QtCore import Property, QObject, Qt, Signal, Slot

from .home import Home
from .models import Collection, CollectionGames, Game, GameListModel, ObjectListModel, collection_key
from .screens import Screens
from .screens.paths import universe_home
from .screens.power import SYSFS, Power
from .themes import ThemeSelector

KEYS = {
    "Accept": (Qt.Key.Key_Return, Qt.Key.Key_Enter),
    "Cancel": (Qt.Key.Key_Escape,),
    "Details": (Qt.Key.Key_I,),
    "Filters": (Qt.Key.Key_F,),
    "PageUp": (Qt.Key.Key_PageUp,),
    "PageDown": (Qt.Key.Key_PageDown,),
    "PrevPage": (Qt.Key.Key_Q,),
    "NextPage": (Qt.Key.Key_E,),
    "Menu": (Qt.Key.Key_F1,),
    "ScreenUp": (Qt.Key.Key_BracketLeft,),
    "ScreenDown": (Qt.Key.Key_BracketRight,),
}

SOURCE_NAMES = {"gog": "GOG", "lutris": "Lutris", "steam": "Steam", "epic": "Epic", "itch": "itch.io"}

PLATFORM_SHORT = {
    "windows": "windows", "linux": "linux", "mac": "mac", "macos": "mac", "steam": "steam",
    "nintendo switch": "switch", "nintendo wii": "wii", "nintendo wii u": "wiiu",
    "nintendo gamecube": "gamecube", "nintendo ds": "nds", "nintendo 3ds": "3ds",
    "sony playstation 2": "ps2", "sony playstation 3": "ps3", "sony playstation 4": "ps4",
    "sony playstation 5": "ps5", "sony playstation portable": "psp", "psp": "psp",
    "sony playstation vita": "vita", "ps vita": "vita", "xbox": "xbox", "microsoft xbox": "xbox",
    "sony playstation": "ps1", "playstation": "ps1", "nintendo game boy advance": "gba", "nintendo game boy": "gb",
    "nintendo 64": "n64", "nintendo snes": "snes", "super nintendo": "snes", "sega dreamcast": "dreamcast",
    "microsoft xbox 360": "xbox360", "xbox 360": "xbox360", "ms-dos": "dos", "dos": "dos", "arcade": "arcade", "scummvm": "scummvm",
}
PLATFORM_NAMES = {"windows": "Windows", "linux": "Linux", "mac": "macOS", "steam": "Steam"}


def _collection_names(key):
    short = PLATFORM_SHORT.get(key.casefold()) or SOURCE_NAMES.get(key, key).casefold().replace(" ", "-")
    name = PLATFORM_NAMES.get(key.casefold()) or SOURCE_NAMES.get(key) or key
    return short, name


def _is(name):
    def test(self, event):
        return event.property("key") in [int(k) for k in KEYS[name]]

    test.__name__ = f"is{name}"
    return Slot(QObject, result=bool)(test)


class Keys(QObject):
    isAccept = _is("Accept")
    isCancel = _is("Cancel")
    isDetails = _is("Details")
    isFilters = _is("Filters")
    isPageUp = _is("PageUp")
    isPageDown = _is("PageDown")
    isPrevPage = _is("PrevPage")
    isNextPage = _is("NextPage")
    isMenu = _is("Menu")
    isScreenUp = _is("ScreenUp")
    isScreenDown = _is("ScreenDown")


class Pad(QObject):
    changed = Signal()
    mutedChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._right_x = 0.0
        self._muted = False

    @Slot(str, float)
    def set(self, name, value):
        if name == "rightX" and self._right_x != value:
            self._right_x = value
            self.changed.emit()

    def setMuted(self, muted):
        if self._muted != bool(muted):
            self._muted = bool(muted)
            self.mutedChanged.emit()

    rightX = Property(float, lambda self: self._right_x, notify=changed)
    muted = Property(bool, lambda self: self._muted, setMuted, notify=mutedChanged)


class Memory(QObject):
    def __init__(self, path=None, parent=None):
        super().__init__(parent)
        if path is None:
            path = os.path.join(universe_home("STATE", ".local/state"), "ui-memory.json")
        self._path = Path(path)
        try:
            with open(self._path) as f:
                self._data = json.load(f)
        except (OSError, ValueError):
            self._data = {}

    def _flush(self):
        try:
            self._path.parent.mkdir(parents=True, exist_ok=True)
            with open(self._path, "w") as f:
                json.dump(self._data, f)
        except OSError:
            pass

    @Slot(str, result="QVariant")
    def get(self, key):
        return self._data.get(key)

    @Slot(str, "QVariant")
    def set(self, key, value):
        if self._data.get(key) == value:
            return
        self._data[key] = value
        self._flush()

    @Slot(str, result=bool)
    def has(self, key):
        return key in self._data

    def unset(self, key):
        if self._data.pop(key, None) is not None:
            self._flush()


class Library(QObject):
    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._games = {}
        self.allGames = GameListModel(self)
        self.collections = ObjectListModel(parent=self)
        self._collection_lists = {}
        client.libraryChanged.connect(self._on_library_changed)
        client.sessionEnded.connect(lambda session_id, ident, duration: self.refresh(ident))
        client.mediaChanged.connect(self.refresh)
        self.reload()

    def reload(self):
        seen = set()
        for data in self._client.list():
            ident = str(data.get("id") or "")
            if not ident or data.get("removed"):
                continue
            seen.add(ident)
            if ident in self._games:
                self._games[ident].update(data)
            else:
                self._games[ident] = Game(data, self, self)
        for ident in list(self._games):
            if ident not in seen:
                self._games.pop(ident).deleteLater()
        self._rebuild()

    def _rebuild(self):
        visible = [g for g in self._games.values() if not g.hidden]
        keys = sorted({collection_key(g) for g in visible if collection_key(g)}, key=lambda k: _collection_names(k)[1])
        collections = []
        for key in keys:
            entry = self._collection_lists.get(key)
            if entry is None:
                proxy = CollectionGames(key, self)
                proxy.setSourceModel(self.allGames)
                short, name = _collection_names(key)
                collection = Collection(short, name, proxy, self)
                entry = (collection, ObjectListModel([collection], self))
                self._collection_lists[key] = entry
            collections.append(entry[0])
        self.allGames.setGames(visible)
        self.collections.setObjects(collections)
        for game in visible:
            entry = self._collection_lists.get(collection_key(game))
            game.setCollections(entry[1] if entry else ObjectListModel([], self))

    @Slot(str)
    def refresh(self, ident):
        data = self._client.game(ident)
        if not data:
            return
        game = self._games.get(ident)
        if game is None:
            self._games[ident] = Game(data, self, self)
            self._rebuild()
            return
        was_hidden = game.hidden
        game.update(data)
        if game.hidden != was_hidden:
            self._rebuild()

    def _on_library_changed(self, ids):
        if not ids:
            self.reload()
        for ident in ids or []:
            self.refresh(ident)

    def get(self, ident):
        return self._games.get(ident)

    def setGameKey(self, ident, key, value):
        self._client.set(ident, key, value)

    def launch(self, game, poster=None):
        self._client.launch(game.id, self.parent().screenName(), poster)


class Api(QObject):
    def __init__(self, client, memory_path=None, fullscreen=False, theme="", power_root=None, parent=None):
        super().__init__(parent)
        self._client = client
        self._keys = Keys(self)
        self._pad = Pad(self)
        self._power = Power(power_root or SYSFS, self)
        self._memory = Memory(memory_path, self)
        self._theme = ThemeSelector(self._memory, theme, self)
        self._library = Library(client, self)
        self._modes = {}
        self._screens = Screens(client, self._memory, self.screenMode, self._library.allGames, self._power, self)
        controller = self._screens.controller
        controller.testingChanged.connect(lambda: self._pad.setMuted(controller.testing))
        self._home = Home(client, controller, self.screenMode, self, frames=lambda: self._theme.frame)
        self._window = None
        self._fullscreen = fullscreen

    def attachWindow(self, window):
        self._window = window
        window.screenChanged.connect(lambda screen: self._modes.clear())

    def shutdown(self):
        self._screens.shutdown()
        self._client.shutdown()

    # Inside gamescope the window's screen is its Xwayland's, not a connector: the profile's default stands.
    def screenName(self):
        if self._client.nested:
            return ""
        screen = self._window.screen() if self._window is not None else None
        return screen.name() if screen is not None else ""

    def screenMode(self):
        name = self.screenName()
        if name not in self._modes:
            self._modes[name] = self._client.screenMode(name)
        return self._modes[name]

    @property
    def library(self):
        return self._library

    keys = Property(QObject, lambda self: self._keys, constant=True)
    pad = Property(QObject, lambda self: self._pad, constant=True)
    power = Property(QObject, lambda self: self._power, constant=True)
    memory = Property(QObject, lambda self: self._memory, constant=True)
    allGames = Property(QObject, lambda self: self._library.allGames, constant=True)
    collections = Property(QObject, lambda self: self._library.collections, constant=True)
    universe = Property(QObject, lambda self: self._client, constant=True)
    screens = Property(QObject, lambda self: self._screens, constant=True)
    theme = Property(QObject, lambda self: self._theme, constant=True)
    home = Property(QObject, lambda self: self._home, constant=True)
    fullscreen = Property(bool, lambda self: self._fullscreen, constant=True)
