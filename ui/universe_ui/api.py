"""The `api` object the theme reads: keys, allGames, collections, memory, universe, screens."""

import json
import os
from pathlib import Path

from PySide6.QtCore import Property, QObject, Qt, Signal, Slot

from .models import Collection, CollectionGames, Game, GameListModel, ObjectListModel, collection_key
from .screens import Screens
from .screens.paths import universe_home

# Pegasus's default keyboard bindings, which tools/shot and the theme's hints assume.
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
}

SOURCE_NAMES = {"gog": "GOG", "lutris": "Lutris", "steam": "Steam", "epic": "Epic", "itch": "itch.io"}

# Platform names as the daemon reports them → the theme's collection shortnames (assets/platforms).
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


def _key_of(event):
    key = event.property("key") if isinstance(event, QObject) else event
    try:
        return int(key)
    except (TypeError, ValueError):
        return -1


class Keys(QObject):
    """`api.keys.isX(event)`: which action a key event stands for. The gamepad posts the same keys."""

    def _is(self, event, name):
        return _key_of(event) in [int(k) for k in KEYS[name]]

    @Slot(QObject, result=bool)
    def isAccept(self, event):
        return self._is(event, "Accept")

    @Slot(QObject, result=bool)
    def isCancel(self, event):
        return self._is(event, "Cancel")

    @Slot(QObject, result=bool)
    def isDetails(self, event):
        return self._is(event, "Details")

    @Slot(QObject, result=bool)
    def isFilters(self, event):
        return self._is(event, "Filters")

    @Slot(QObject, result=bool)
    def isPageUp(self, event):
        return self._is(event, "PageUp")

    @Slot(QObject, result=bool)
    def isPageDown(self, event):
        return self._is(event, "PageDown")

    @Slot(QObject, result=bool)
    def isPrevPage(self, event):
        return self._is(event, "PrevPage")

    @Slot(QObject, result=bool)
    def isNextPage(self, event):
        return self._is(event, "NextPage")

    @Slot(QObject, result=bool)
    def isMenu(self, event):
        return self._is(event, "Menu")


class Pad(QObject):
    """`api.pad`: the sticks the theme reads as values; 0 with no controller. `muted` keeps the pad's
    presses from becoming keys while the controller section shows them live."""

    changed = Signal()
    mutedChanged = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._axes = {"rightX": 0.0, "rightY": 0.0}
        self._muted = False

    @Slot(str, float)
    def set(self, name, value):
        if name in self._axes and self._axes[name] != value:
            self._axes[name] = value
            self.changed.emit()

    @Slot(bool)
    def setMuted(self, muted):
        if self._muted != bool(muted):
            self._muted = bool(muted)
            self.mutedChanged.emit()

    rightX = Property(float, lambda self: self._axes["rightX"], notify=changed)
    rightY = Property(float, lambda self: self._axes["rightY"], notify=changed)
    muted = Property(bool, lambda self: self._muted, setMuted, notify=mutedChanged)


class Memory(QObject):
    """`api.memory`: small persisted key/value store, $UNIVERSE_STATE_HOME/ui-memory.json."""

    def __init__(self, path=None, parent=None):
        super().__init__(parent)
        if path is None:
            path = os.path.join(universe_home("STATE", ".local/state"), "ui-memory.json")
        self._path = Path(path)
        self._data = {}
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

    @Slot(str)
    def unset(self, key):
        if self._data.pop(key, None) is not None:
            self._flush()


class Library(QObject):
    """Owns the Game objects; keeps them current from Library1 and the daemon's signals."""

    loaded = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._api = parent
        self._games = {}
        self.allGames = GameListModel(self)
        self.collections = ObjectListModel(parent=self)
        self._collection_lists = {}
        client.libraryChanged.connect(self._on_library_changed)
        client.sessionEnded.connect(lambda session_id, ident, duration: self.refresh(ident))
        client.mediaChanged.connect(self.refresh)
        self.reload()

    def reload(self):
        rows = self._client.list() or []
        seen = set()
        for data in rows:
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
        self.loaded.emit()

    # Hidden games stay out of every model: the theme has no notion of them. Collections are
    # platforms (the theme labels them by shortname), sorted by name.
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
        was_hidden = game.hidden if game else None
        if game is None:
            self._games[ident] = Game(data, self, self)
            self._rebuild()
            return
        game.update(data)
        if game.hidden != was_hidden:
            self._rebuild()

    def _on_library_changed(self, ids):
        ids = list(ids or [])
        if not ids:
            self.reload()
            return
        for ident in ids:
            self.refresh(ident)

    def get(self, ident):
        return self._games.get(ident)

    def setGameKey(self, ident, key, value):
        self._client.set(ident, key, value)

    def launch(self, game):
        self._client.launch(game.id, self._api.screenName() if self._api else "")


class Api(QObject):
    fullscreenChanged = Signal()

    def __init__(self, client, memory_path=None, fullscreen=False, parent=None):
        super().__init__(parent)
        self._client = client
        self._keys = Keys(self)
        self._pad = Pad(self)
        self._memory = Memory(memory_path, self)
        self._library = Library(client, self)
        self._screens = Screens(client, self.screenHz, self, memory=self._memory)
        controller = self._screens.controller
        controller.testingChanged.connect(lambda: self._pad.setMuted(controller.testing))
        self._window = None
        self._fullscreen = fullscreen

    def attachWindow(self, window):
        self._window = window

    def shutdown(self):
        self._screens.shutdown()
        if hasattr(self._client, "shutdown"):
            self._client.shutdown()

    def screenName(self):
        window = self._window
        screen = window.screen() if window is not None else None
        return screen.name() if screen is not None else ""

    def screenHz(self):
        """The refresh rate of the screen the window is on, 0 when there is none yet."""
        window = self._window
        screen = window.screen() if window is not None else None
        return int(round(screen.refreshRate())) if screen is not None else 0

    @property
    def library(self):
        return self._library

    keys = Property(QObject, lambda self: self._keys, constant=True)
    pad = Property(QObject, lambda self: self._pad, constant=True)
    memory = Property(QObject, lambda self: self._memory, constant=True)
    allGames = Property(QObject, lambda self: self._library.allGames, constant=True)
    collections = Property(QObject, lambda self: self._library.collections, constant=True)
    universe = Property(QObject, lambda self: self._client, constant=True)
    screens = Property(QObject, lambda self: self._screens, constant=True)
    fullscreen = Property(bool, lambda self: self._fullscreen, notify=fullscreenChanged)
