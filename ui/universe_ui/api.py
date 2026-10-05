import json
import logging
import os
from pathlib import Path
from typing import cast

from PySide6.QtCore import QEvent, QObject, Qt, QTimer, Signal, Slot

from . import keyboard
from .boot import Boot
from .focus import Focus
from .home import Home
from .models import Collection, CollectionGames, Game, GameListModel, ObjectListModel, collection_key
from .qt import Property
from .screens import Screens
from .screens.network import Network
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


def from_touch(event):
    """Qt's mouse press or move made from an unhandled touch: no mouse moved."""
    from PySide6.QtGui import QInputDevice

    device = event.device() if hasattr(event, "device") else None
    return device is not None and device.type() == QInputDevice.DeviceType.TouchScreen


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


# B held this long opens the power menu: the A-hold that opens a game's menu.
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
    def __init__(self, parent=None, boot=None):
        super().__init__(parent)
        self._boot = boot
        self._hold = QTimer(self)
        self._hold.setSingleShot(True)
        self._hold.setInterval(CANCEL_HOLD_MS)
        self._hold.timeout.connect(self._held)
        self._mode = "pad"
        self._windows = []
        self._layout = {"name": "", "rows": {}}

    def _held(self):
        if os.environ.get("UNIVERSE_UI_INPUT_LOG"):
            logging.getLogger("universe.keys").info("cancel held")
        self.cancelHeld.emit()

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
            if self._boot_takes(int(event.key()), kind == QEvent.Type.KeyPress):
                return True
            if not event.isAutoRepeat() and event.key() in HOLD_KEYS:
                if kind == QEvent.Type.KeyPress:
                    self._hold.start()
                else:
                    self._hold.stop()
        elif kind in (QEvent.Type.TouchBegin, QEvent.Type.TouchUpdate):
            # A finger on a Deck's screen: the pad's glyphs in the hints, no cursor, no hover.
            self._set_mode("pad", event)
            return self._boot_takes("touch", True)
        elif kind in (QEvent.Type.TouchEnd, QEvent.Type.TouchCancel):
            return self._boot_takes("touch", False)
        elif kind in (QEvent.Type.MouseMove, QEvent.Type.MouseButtonPress, QEvent.Type.Wheel) and not from_touch(event):
            self._set_mode("mouse", event)
            if kind == QEvent.Type.MouseButtonPress:
                return self._boot_takes("mouse", True)
            if kind == QEvent.Type.Wheel and self._boot is not None and self._boot.running:
                self._boot.skip()
                return True
        elif kind in (QEvent.Type.MouseButtonRelease, QEvent.Type.MouseButtonDblClick):
            return self._boot_takes("mouse", kind == QEvent.Type.MouseButtonDblClick)
        return False

    def _boot_takes(self, ident, pressed):
        return self._boot is not None and self._boot.takes(ident, pressed)

    # "pad" | "keyboard" | "mouse": whatever was used last, a touch counting as the pad. The hints read it; a hover counts only under a mouse.
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


class System(QObject):
    changed = Signal()
    failed = Signal(str, str)

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._actions = []
        self._steam = bool(client.underSteam)
        self._session = bool(client.session)
        self._deck = str(client.deck)
        self._controls = []
        self._radios = (False, False)
        client.powerActionsAsync(self._set_actions)
        # What `[system]` kept goes back first: the power limit and the clocks do not outlive a reboot.
        client.applySystemAsync(self.reload)
        # A launch puts the game's own controls on and its end takes them off.
        client.sessionStarted.connect(lambda *_: self.reload())
        client.sessionEnded.connect(lambda *_: self.reload())

    def _set_actions(self, ids):
        self._actions = ids
        self.changed.emit()

    # "suspend" | "reboot" | "power_off"; `failed(action, message)` when logind refuses.
    @Slot(str)
    def run(self, action):
        self._client.powerAsync(action, lambda e: self.failed.emit(action, e.message or e.kind))

    @Slot()
    def reload(self):
        self._client.systemControlsAsync(self._set_controls)

    def _set_controls(self, rows):
        self._controls = rows
        self.controlsChanged.emit()

    # The row shows the value at once; a refusal puts the machine's own back and says why.
    def _shown(self, ident, value):
        self._controls = [{**c, "value": value} if c["id"] == ident else c for c in self._controls]
        self.controlsChanged.emit()

        def refused(e):
            self.controlFailed.emit(ident, e.message or e.kind)
            self.reload()

        return refused

    # The machine's own value.
    @Slot(str, str)
    def set(self, ident, value):
        self._client.setSystemAsync(ident, value, lambda: None, self._shown(ident, value))

    # The game's own while `game` names one, put on by its launches; else the machine's own.
    @Slot(str, str, str)
    def setFor(self, game, ident, value):
        if not game:
            self.set(ident, value)
            return
        self._client.setSystemForAsync(game, ident, value, lambda: None, self._shown(ident, value))

    # The machine's own value, which `game` then follows instead of its own.
    @Slot(str, str, str)
    def setAll(self, game, ident, value):
        self._client.setSystemAllAsync(game, ident, value, lambda: None, self._shown(ident, value))

    @Slot(str, result="QVariant")
    def control(self, ident):
        return next((c for c in self._controls if c["id"] == ident), None)

    def setRadios(self, network, bluetooth):
        radios = (network and not self._steam, bluetooth and not self._steam)
        if radios != self._radios:
            self._radios = radios
            self.radiosChanged.emit()

    radiosChanged = Signal()
    # NetworkManager lists a Wi-Fi card, BlueZ an adapter, and Steam's Game Mode, which has its own, is not around: the pages show.
    network = Property(bool, lambda self: self._radios[0], notify=radiosChanged)
    bluetooth = Property(bool, lambda self: self._radios[1], notify=radiosChanged)

    controlsChanged = Signal()
    controlFailed = Signal(str, str)
    actions = Property("QStringList", lambda self: self._actions, notify=changed)
    # [{id, label, detail, kind: range | choice | toggle, value, min, max, step, unit, choices}]: see the core's `hardware::Control`.
    controls = Property("QVariantList", lambda self: self._controls, notify=controlsChanged)
    # Inside Steam's Game Mode: power, sound, screenshots and the HUD are Steam's, and no HOME reaches the launcher over a game.
    steam = Property(bool, lambda self: self._steam, constant=True)
    # The Universe session a display manager started: quitting the launcher logs out.
    session = Property(bool, lambda self: self._session, constant=True)
    # "lcd" | "oled" on a Steam Deck, else "".
    deck = Property(str, lambda self: self._deck, constant=True)


class Api(QObject):
    # boot: whether this start may open on the startup animation; Settings › Themes can still turn it off.
    def __init__(self, client, memory_path=None, fullscreen=False, theme="", power_root=None, net_root=None, boot=False, parent=None):
        super().__init__(parent)
        self._client = client
        self._memory = Memory(memory_path, self)
        self._theme = ThemeSelector(self._memory, theme, self)
        self._boot = Boot(boot and self._theme.bootIntro, self)
        self._keys = Keys(self, boot=self._boot)
        self._keys.setLayout(keyboard.rows(**client.keyboardLayout()))
        self._pad = Pad(self)
        self._power = Power(power_root or SYSFS, self)
        self._network = Network(os.path.join(net_root, "class"), os.path.join(net_root, "wireless"), self) if net_root else Network(parent=self)
        self._system = System(client, self)
        self._library = Library(client, self)
        self._modes = {}
        self._screens = Screens(
            client,
            self._memory,
            self.screenMode,
            self._library.allGames,
            self._power,
            self._theme_list,
            network=self._network,
            busy=lambda: self._home.shown == "game",
            parent=self,
        )
        radios = (self._screens.network, self._screens.bluetooth)
        for radio in radios:
            radio.changed.connect(lambda: self._system.setRadios(*(r.available for r in radios)))
        controller = self._screens.controller
        controller.testingChanged.connect(lambda: self._pad.setMuted(controller.testing or controller.walking))
        controller.walkChanged.connect(lambda: self._pad.setMuted(controller.testing or controller.walking))
        self._focus = Focus(client, parent=self)
        self._focus.changed.connect(self._on_focus)
        self._home = Home(client, controller, self.screenMode, self, frames=lambda: self._theme.frame, focus=self._focus, boot=self._boot)
        self._window = None
        self._fullscreen = fullscreen

    # B held as another app takes the focus never sees its release here: the power menu would open behind it.
    def _on_focus(self):
        if not self._focus.active:
            self._keys.dropHold()
            self._screens.controller.focusLost()

    def attachWindow(self, window):
        self._window = window
        self._keys.watch(window)
        self._boot.watch(window)
        window.screenChanged.connect(lambda screen: self._modes.clear())

    # The network watch follows the link for the status icons from the start; in the Universe session the Bluetooth one does too,
    # as the default agent a Sony pad plugged in by cable asks. Elsewhere it starts with a page. Steam's Game Mode keeps both.
    def startRadios(self):
        if self._client.underSteam:
            return
        network, bluetooth = self._screens.network, self._screens.bluetooth

        def start():
            if network.available:
                network.start()
            if bluetooth.available and self._client.session:
                bluetooth.start()

        network.changed.connect(start)
        bluetooth.changed.connect(start)
        start()

    def shutdown(self):
        self._focus.shutdown()
        self._home.shutdown()
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
    focus = Property(QObject, lambda self: self._focus, constant=True)
    power = Property(QObject, lambda self: self._power, constant=True)
    network = Property(QObject, lambda self: self._network, constant=True)
    system = Property(QObject, lambda self: self._system, constant=True)

    def _theme_list(self):
        return [{**t, "current": t["id"] == self._theme.current} for t in self._theme.themes]

    memory = Property(QObject, lambda self: self._memory, constant=True)
    allGames = Property(QObject, lambda self: self._library.allGames, constant=True)
    collections = Property(QObject, lambda self: self._library.collections, constant=True)
    universe = Property(QObject, lambda self: self._client, constant=True)
    screens = Property(QObject, lambda self: self._screens, constant=True)
    theme = Property(QObject, lambda self: self._theme, constant=True)
    home = Property(QObject, lambda self: self._home, constant=True)
    boot = Property(QObject, lambda self: self._boot, constant=True)
    fullscreen = Property(bool, lambda self: self._fullscreen, constant=True)
