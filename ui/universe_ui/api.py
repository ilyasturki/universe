import json
import logging
import os
from pathlib import Path
from typing import cast

from PySide6.QtCore import QEvent, QObject, Qt, QTimer, Signal, Slot

from . import keyboard
from .home import Home
from .models import Collection, CollectionGames, Game, GameListModel, ObjectListModel, collection_key
from .qt import Property
from .screens import Screens
from .screens.paths import universe_home
from .screens.power import SYSFS, Power
from .themes import ThemeSelector

KEYS = {
    "Accept": (Qt.Key.Key_Return, Qt.Key.Key_Enter),
    "Cancel": (Qt.Key.Key_Escape, Qt.Key.Key_Backspace),
    "Details": (Qt.Key.Key_I,),
    "Filters": (Qt.Key.Key_F,),
    "PageUp": (Qt.Key.Key_PageUp,),
    "PageDown": (Qt.Key.Key_PageDown,),
    "PrevPage": (Qt.Key.Key_Q,),
    "NextPage": (Qt.Key.Key_E,),
    "Menu": (Qt.Key.Key_F1,),
    "ScreenUp": (Qt.Key.Key_BracketLeft,),
    "ScreenDown": (Qt.Key.Key_BracketRight,),
    "First": (Qt.Key.Key_Home,),
    "Last": (Qt.Key.Key_End,),
    "Up": (Qt.Key.Key_Up,),
    "Down": (Qt.Key.Key_Down,),
    "Left": (Qt.Key.Key_Left,),
    "Right": (Qt.Key.Key_Right,),
}

# The pad button each action sits on, as the hints name it; under a keyboard the hint shows the action's first key instead.
GLYPH_ACTIONS = {
    "A": "Accept",
    "B": "Cancel",
    "X": "Details",
    "Y": "Filters",
    "LB": "PrevPage",
    "RB": "NextPage",
    "LT": "PageUp",
    "RT": "PageDown",
    "Start": "Menu",
}
KEY_LABELS = {Qt.Key.Key_Return: "Enter", Qt.Key.Key_Escape: "Esc", Qt.Key.Key_PageUp: "PgUp", Qt.Key.Key_PageDown: "PgDn"}


def describe_event(event):
    if event is None:
        return "?"
    parts = [str(event.type()).rsplit(".", 1)[-1], "spontaneous" if event.spontaneous() else "posted"]
    if hasattr(event, "nativeScanCode"):
        parts.append(
            f"key={int(event.key())} text={event.text()!r} scan={event.nativeScanCode()} vkey={event.nativeVirtualKey()} repeat={event.isAutoRepeat()}"
        )
    if hasattr(event, "globalPosition"):
        parts.append(f"pos={event.position().toPoint().toTuple()} global={event.globalPosition().toPoint().toTuple()}")
    if hasattr(event, "device") and event.device() is not None:
        dev = event.device()
        parts.append(f"device={dev.name()!r} type={str(dev.type()).rsplit('.', 1)[-1]} seat={dev.seatName()!r}")
    return " ".join(parts)


def key_label(key):
    from PySide6.QtGui import QKeySequence

    return KEY_LABELS.get(key) or QKeySequence(key).toString()


# B held this long asks to quit the launcher: the A-hold that opens a game's menu.
CANCEL_HOLD_MS = 450
# Escape only: Backspace held in a text sheet is deleting, not leaving.
HOLD_KEYS = (Qt.Key.Key_Escape,)

SOURCE_NAMES = {"gog": "GOG", "lutris": "Lutris", "steam": "Steam", "epic": "Epic", "itch": "itch.io"}

PLATFORM_SHORT = {
    "windows": "windows",
    "linux": "linux",
    "mac": "mac",
    "macos": "mac",
    "steam": "steam",
    "nintendo switch": "switch",
    "nintendo wii": "wii",
    "nintendo wii u": "wiiu",
    "nintendo gamecube": "gamecube",
    "nintendo ds": "nds",
    "nintendo 3ds": "3ds",
    "sony playstation 2": "ps2",
    "sony playstation 3": "ps3",
    "sony playstation 4": "ps4",
    "sony playstation 5": "ps5",
    "sony playstation portable": "psp",
    "psp": "psp",
    "sony playstation vita": "vita",
    "ps vita": "vita",
    "xbox": "xbox",
    "microsoft xbox": "xbox",
    "sony playstation": "ps1",
    "playstation": "ps1",
    "nintendo game boy advance": "gba",
    "nintendo game boy": "gb",
    "nintendo 64": "n64",
    "nintendo snes": "snes",
    "super nintendo": "snes",
    "sega dreamcast": "dreamcast",
    "microsoft xbox 360": "xbox360",
    "xbox 360": "xbox360",
    "ms-dos": "dos",
    "dos": "dos",
    "arcade": "arcade",
    "scummvm": "scummvm",
}
PLATFORM_NAMES = {"windows": "Windows", "linux": "Linux", "mac": "macOS", "steam": "Steam"}


def _collection_names(key: str):
    short = PLATFORM_SHORT.get(key.casefold()) or SOURCE_NAMES.get(key, key).casefold().replace(" ", "-")
    name = PLATFORM_NAMES.get(key.casefold()) or SOURCE_NAMES.get(key) or key
    return short, name


def _is(name):
    def test(self, event):
        return event.property("key") in [int(k) for k in KEYS[name]]

    test.__name__ = f"is{name}"
    return Slot(QObject, result=bool)(test)


class Keys(QObject):
    cancelHeld = Signal()
    modeChanged = Signal()

    # Watches the window's own key events, so the hold counts whatever page has the focus and however it takes B.
    def __init__(self, parent=None):
        super().__init__(parent)
        self._hold = QTimer(self)
        self._hold.setSingleShot(True)
        self._hold.setInterval(CANCEL_HOLD_MS)
        self._hold.timeout.connect(self.cancelHeld)
        self._mode = "pad"
        self._windows = []
        self._layout = {"name": "", "rows": {}}

    def watch(self, window):
        self._windows.append(window)
        window.installEventFilter(self)
        self._cursor(window)

    # A question B just closed: holding on does not ask again.
    @Slot()
    def dropHold(self):
        self._hold.stop()

    # A click's A or B, the wheel's step: the same key the pad posts, so a page needs no second path.
    @Slot(str)
    def press(self, action):
        self.hold(action)
        self.release(action)

    @Slot(str)
    def hold(self, action):
        from .gamepad import post_key

        post_key(KEYS[action][0], True, window=self._focus_window(), source="pointer")

    @Slot(str)
    def release(self, action):
        from .gamepad import post_key

        post_key(KEYS[action][0], False, window=self._focus_window(), source="pointer")

    def _focus_window(self):
        from PySide6.QtGui import QGuiApplication

        return QGuiApplication.focusWindow() or (self._windows[0] if self._windows else None)

    def _set_mode(self, mode, event=None):
        if mode == self._mode:
            return
        if os.environ.get("UNIVERSE_UI_INPUT_LOG"):
            logging.getLogger("universe.keys").info("mode %s -> %s on %s", self._mode, mode, describe_event(event))
        self._mode = mode
        for window in self._windows:
            self._cursor(window)
        self.modeChanged.emit()

    # gamescope's default cursor is GNOME's X cursor at scale 1, twice the size on a 2× screen; Qt's arrow is the logical size.
    def _cursor(self, window):
        from PySide6.QtGui import QCursor

        window.setCursor(QCursor(Qt.CursorShape.ArrowCursor if self._mode == "mouse" else Qt.CursorShape.BlankCursor))

    def eventFilter(self, obj, event):
        from .gamepad import posted_source

        kind = event.type()
        if kind in (QEvent.Type.KeyPress, QEvent.Type.KeyRelease):
            source = posted_source(event.key(), kind == QEvent.Type.KeyPress)
            if source == "pad":
                self._set_mode("pad", event)
            # A key with no keysym is not typing: InputPlumber's keyboard target sends KEY_UNKNOWN for every pad button.
            elif source is None and event.key() not in (0, Qt.Key.Key_unknown):
                self._set_mode("keyboard", event)
            if not event.isAutoRepeat() and event.key() in HOLD_KEYS:
                if kind == QEvent.Type.KeyPress:
                    self._hold.start()
                else:
                    self._hold.stop()
        elif kind in (QEvent.Type.MouseMove, QEvent.Type.MouseButtonPress, QEvent.Type.Wheel):
            self._set_mode("mouse", event)
        return False

    # "pad" | "keyboard" | "mouse": whatever was used last. The hints read it; a hover counts only under a mouse.
    mode = Property(str, lambda self: self._mode, notify=modeChanged)
    # Pad glyph → key label ("A" → "Enter"), for the hints under a keyboard.
    labels = Property("QVariantMap", lambda self: {glyph: key_label(KEYS[action][0]) for glyph, action in GLYPH_ACTIONS.items()}, constant=True)
    # The physical keyboard's rows for the on-screen ones (`keyboard.rows`): `name`, `rows` (`AE`, `AD`, `AC`, `AB` of `{value, shift}`).
    layout = Property("QVariantMap", lambda self: self._layout, constant=True)

    def setLayout(self, layout):
        self._layout = layout

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
    isFirst = _is("First")
    isLast = _is("Last")


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
        keys = sorted({key for g in visible if (key := collection_key(g))}, key=lambda k: _collection_names(k)[1])
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
        api = cast("Api", self.parent())
        self._client.launch(game.id, api.screenName(), poster)


class Api(QObject):
    def __init__(self, client, memory_path=None, fullscreen=False, theme="", power_root=None, parent=None):
        super().__init__(parent)
        self._client = client
        self._keys = Keys(self)
        self._keys.setLayout(keyboard.rows(**client.keyboardLayout()))
        self._pad = Pad(self)
        self._power = Power(power_root or SYSFS, self)
        self._memory = Memory(memory_path, self)
        self._theme = ThemeSelector(self._memory, theme, self)
        self._library = Library(client, self)
        self._modes = {}
        self._screens = Screens(client, self._memory, self.screenMode, self._library.allGames, self._power, self._theme_list, self)
        controller = self._screens.controller
        controller.testingChanged.connect(lambda: self._pad.setMuted(controller.testing or controller.walking))
        controller.walkChanged.connect(lambda: self._pad.setMuted(controller.testing or controller.walking))
        self._home = Home(client, controller, self.screenMode, self, frames=lambda: self._theme.frame)
        self._window = None
        self._fullscreen = fullscreen

    def attachWindow(self, window):
        self._window = window
        self._keys.watch(window)
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

    def _theme_list(self):
        return [{**t, "current": t["id"] == self._theme.current} for t in self._theme.themes]

    memory = Property(QObject, lambda self: self._memory, constant=True)
    allGames = Property(QObject, lambda self: self._library.allGames, constant=True)
    collections = Property(QObject, lambda self: self._library.collections, constant=True)
    universe = Property(QObject, lambda self: self._client, constant=True)
    screens = Property(QObject, lambda self: self._screens, constant=True)
    theme = Property(QObject, lambda self: self._theme, constant=True)
    home = Property(QObject, lambda self: self._home, constant=True)
    fullscreen = Property(bool, lambda self: self._fullscreen, constant=True)
