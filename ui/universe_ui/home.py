import os
import time

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot

OPAQUE = 0xFFFFFFFF
POLL_MS = 250
HOLD_MS = 600
# The overlay stays painted over the game this long for the flash the theme draws on a shot.
CUE_MS = 450
SHUTTER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "qml", "assets", "sounds", "shutter.wav")
# How long the swap waits for the theme to say the frame is painted (`covered`) before going ahead anyway: a 4K png decodes slowly.
COVER_MS = 700
# A frame taken at the press still stands for a flip this much later while the game runs on under the dock.
FRESH_S = 1.0


class Home(QObject):
    pressed = Signal()
    changed = Signal()
    volumeChanged = Signal()
    screenshotTaken = Signal(str)
    stopping = Signal(str)

    def __init__(self, client, controller, screen_mode=dict, parent=None, frames=lambda: True):
        super().__init__(parent)
        self._client = client
        self._controller = controller
        self._screen_mode = screen_mode
        self._frames = frames
        self._overlay = None
        self._open = False
        self._closing = False
        self._shown = "launcher"
        self._flipped = False
        self._paused = False
        self._pause_on_home = False
        self._held = False
        self._suspended = False
        self._thaw_on_release = False
        self._stopping = False
        self._flipping = False
        self._keys_on_thaw = []
        self._frame = ""
        self._landing = ""
        self._taken = 0
        self._capturing = False
        self._captured = None
        self._captured_at = 0.0
        self._captured_still = False
        self._volume = {}
        self._poll = QTimer(self)
        self._poll.setInterval(POLL_MS)
        self._poll.timeout.connect(self._refresh)
        self._hold = QTimer(self)
        self._hold.setSingleShot(True)
        self._hold.timeout.connect(self._on_hold)
        self._hold_from_game = False
        self._swap = QTimer(self)
        self._swap.setSingleShot(True)
        self._swap.timeout.connect(self._show_launcher)
        self._cue = QTimer(self)
        self._cue.setSingleShot(True)
        self._cue.setInterval(CUE_MS)
        self._cue.timeout.connect(self._cue_done)
        self._shutter = None
        client.currentSessionChanged.connect(self._on_session)
        client.sessionShown.connect(self._on_shown)
        controller.buttonPressed.connect(self._on_button)
        controller.screenshotTaken.connect(self._shot_taken)
        self._on_session()

    def attachOverlay(self, window):
        self._overlay = window
        if not self._client.nested:
            return False
        self._client.overlay(window.winId(), False, 0)
        window.show()
        return True

    def _overlay_state(self, input, opacity):
        if self._overlay is not None and self._client.nested:
            self._client.overlay(self._overlay.winId(), input, opacity)

    def _session(self):
        current = self._client.currentSession
        return current if current and current.get("session_id") else None

    def _game(self):
        session = self._session()
        return self._client.game(str(session.get("id") or "")) if session else {}

    def _on_session(self):
        if self._session():
            self._pause_on_home = bool((self._game().get("effective") or {}).get("pause_on_home"))
            if self._client.nested:
                self._poll.start()
                self._refresh()
        else:
            self._poll.stop()
            self._swap.stop()
            self._drop()
            self._open = self._closing = self._flipped = self._flipping = self._paused = self._thaw_on_release = self._stopping = False
            self._keys_on_thaw = []
            self._shown = "launcher"
            self._frame = ""
            self._landing = ""
            self._captured = None
        self.changed.emit()

    def _on_shown(self, session_id, ok):
        if ok and not self._client.nested and self._session():
            self._shown = "game"
            self.changed.emit()

    def _refresh(self):
        shown = "game" if self._client.gameShown() else "launcher"
        if shown != self._shown:
            self._shown = shown
            self.changed.emit()

    def _on_button(self, ident, slot, pressed):
        if slot == "guide":
            self.guide(pressed)

    @Slot(bool)
    def guide(self, pressed):
        self._held = bool(pressed)
        if pressed:
            self._hold_from_game = self._shown == "game" and self._session() is not None
            if self._hold_from_game:
                self._capture()
            self._hold.start(int((self._controller.state or {}).get("hold_ms") or HOLD_MS))
            self.pressed.emit()
        else:
            self._hold.stop()
            if self._thaw_on_release:
                self._thaw_on_release = False
                self._set_paused(False)
        self.changed.emit()

    def _on_hold(self):
        if self._hold_from_game and self._shown == "game":
            self.toLauncher()

    @Slot()
    def openDock(self):
        if self._open or not self._session():
            return
        if self._overlay is None:
            self.toLauncher()
            return
        self._open, self._closing = True, False
        self._overlay_state(True, OPAQUE)
        self._suspend()
        if self._pause_on_home:
            self._set_paused(True)
        self.changed.emit()

    def _suspend(self):
        if not self._suspended:
            self._suspended = True
            self._controller.suspend()

    @Slot()
    def closeDock(self):
        if not self._open:
            return
        self._open, self._closing = False, True
        self.changed.emit()

    @Slot()
    def dockClosed(self):
        if not self._closing:
            return
        self._closing = False
        self._drop()
        if self._paused and self._shown == "game" and not self._flipping:
            self._thaw()

    def _drop(self):
        self._overlay_state(False, 0)
        if self._suspended:
            self._suspended = False
            self._controller.resume()

    def _thaw(self):
        if self._held:
            self._thaw_on_release = True
        else:
            self._set_paused(False)

    @Slot()
    def toGame(self):
        if not self._session():
            return
        if self._swap.isActive():
            self._swap.stop()
            if self._client.nested:
                self._poll.start()
        if self._open:
            self.closeDock()
        self._client.focusSession()
        self._flipped = False
        self._landing = ""
        self._captured = None
        if self._paused:
            self._thaw()
        self._shown = "game"
        self.changed.emit()

    # Asked at the press so the flip need not wait: gamescope takes up to 2 s at 4K.
    def _capture(self):
        if self._capturing or not self._client.nested or not self._frames():
            return
        self._capturing = True
        self._captured = None
        self._client.frame(self._captured_done)

    def _captured_done(self, path):
        self._capturing = False
        if not self._session():
            return
        self._captured = path
        self._captured_at = time.monotonic()
        self._captured_still = self._paused
        if self._flipping:
            self._flip(path)

    # A frozen game has painted nothing since: its frame stands however old.
    def _fresh(self):
        return self._captured is not None and (self._captured_still or time.monotonic() - self._captured_at < FRESH_S)

    # `landing` names the page the theme opens on the playing game once it is up: details, journal, recordings, screenshots.
    @Slot()
    @Slot(str)
    def toLauncher(self, landing=""):
        if not self._session() or self._flipping:
            return
        self._landing = str(landing or "")
        if self._open:
            self.closeDock()
        if not self._client.nested or not self._frames():
            self._flip("")
            return
        self._flipping = True
        if self._fresh():
            self._flip(self._captured)
        else:
            self._capture()

    def _flip(self, path):
        self._flipping = False
        self._captured = None
        if not self._session():
            return
        if path:
            # The same file every time: the query keeps the image cache from showing the previous frame.
            self._taken += 1
            self._frame = QUrl.fromLocalFile(path).toString() + "?" + str(self._taken)
        else:
            self._frame = ""
        self._shown = "launcher"
        self._flipped = True
        self._thaw_on_release = False
        # Armed before `changed`: the theme answers `covered()` once the frame is painted.
        self._poll.stop()
        if path:
            self._swap.start(COVER_MS)
        else:
            self._show_launcher()
        if self._pause_on_home:
            self._set_paused(True)
        self.changed.emit()

    @Slot(result=str)
    def takeLanding(self):
        landing, self._landing = self._landing, ""
        return landing

    @Slot()
    def covered(self):
        if self._swap.isActive():
            self._swap.stop()
            self._show_launcher()

    def _show_launcher(self):
        self._client.focusLauncher()
        session = self._session()
        if not session:
            return
        if self._stopping:
            self._client.stop(str(session.get("session_id") or ""))
        if self._client.nested:
            self._poll.start()

    # The launcher comes up first: the game is stopped once the frame is taken, since a quitting one paints nothing.
    @Slot()
    def stop(self):
        session = self._session()
        if not session or self._stopping:
            return
        self._stopping = True
        self.stopping.emit(str(session.get("title") or ""))
        if self._shown == "game" or self._open:
            self.toLauncher()
        else:
            self._client.stop(str(session.get("session_id") or ""))

    def _set_paused(self, on):
        if on == self._paused or not self._session() or (on and self._stopping):
            return
        self._paused = on
        if not on:
            self._captured_still = False
        self._client.freeze(on, None if on else lambda _: self._type_held())
        self.changed.emit()

    # A frozen game reads no key: what the dock asks of the game's MangoHud waits for the thaw.
    def _type(self, combo):
        if self._paused:
            if combo not in self._keys_on_thaw:
                self._keys_on_thaw.append(combo)
        else:
            self._controller.run("keys", combo)

    def _type_held(self):
        keys, self._keys_on_thaw = self._keys_on_thaw, []
        for combo in keys:
            self._controller.run("keys", combo)

    @Slot(bool)
    def setPauseOnHome(self, on):
        session = self._session()
        if not session:
            return
        self._pause_on_home = bool(on)
        self._client.set(str(session.get("id") or ""), "launch.pause_on_home", "true" if on else "false")
        if self._open or self._flipped:
            self._set_paused(bool(on))
        self.changed.emit()

    @Slot()
    def screenshot(self):
        self._client.runAsync(self._client.screenshot, lambda path: self._shot_taken(str(path or "")))

    # The hook answers after the grab: the cue never lands in the shot.
    def _shot_taken(self, path):
        if path:
            self._play_shutter()
            if not self._open and not self._closing and self._shown == "game":
                self._overlay_state(False, OPAQUE)
                self._cue.start()
        self.screenshotTaken.emit(path)

    def _cue_done(self):
        if not self._open and not self._closing:
            self._overlay_state(False, 0)

    def _play_shutter(self):
        if self._shutter is None:
            try:
                from PySide6.QtMultimedia import QSoundEffect
            except ImportError:
                self._shutter = False
                return
            self._shutter = QSoundEffect(self)
            self._shutter.setSource(QUrl.fromLocalFile(SHUTTER))
        if self._shutter:
            self._shutter.play()

    @Slot(str, result=str)
    def launchValue(self, key):
        value = (self._game().get("effective") or {}).get(key)
        if isinstance(value, bool):
            return "true" if value else "false"
        return "" if value is None else str(value)

    @Slot(str, result="QVariantList")
    def launchChoices(self, key):
        for spec in self._client.launchKeys("both", self._screen_mode()):
            if spec.get("key") == key:
                return [str(c) for c in spec.get("choices") or []]
        return []

    @Slot(str, str)
    def setLaunchValue(self, key, value):
        session = self._session()
        if not session:
            return
        if key == "mangohud":
            self._client.setMangohud(value == "true")
            self.changed.emit()
            return
        self._client.set(str(session.get("id") or ""), "launch." + key, value)
        if key == "fps_limit":
            self._client.setFpsLimit(lambda combo: combo and self._type(combo))
        elif key == "gamescope_filter" and self._client.nested:
            sharpness = (self._game().get("effective") or {}).get("gamescope_sharpness")
            self._client.nestFilter(value, None if sharpness in (None, "") else int(sharpness))
        self.changed.emit()

    @Slot(result=int)
    def screenRefresh(self):
        return int(self._screen_mode().get("refresh") or 0)

    @Slot(str, int)
    def volume(self, change, value=0):
        def landed(level):
            level = dict(level or {})
            if level != self._volume:
                self._volume = level
                self.volumeChanged.emit()

        self._client.volumeAsync(change, int(value), landed)

    shown = Property(str, lambda self: self._shown, notify=changed)
    open = Property(bool, lambda self: self._open, notify=changed)
    paused = Property(bool, lambda self: self._paused, notify=changed)
    pauseOnHome = Property(bool, lambda self: self._pause_on_home, notify=changed)
    flipped = Property(bool, lambda self: self._flipped, notify=changed)
    frame = Property(str, lambda self: self._frame, notify=changed)
    volumePercent = Property(int, lambda self: int(self._volume.get("percent") or 0), notify=volumeChanged)
    muted = Property(bool, lambda self: bool(self._volume.get("muted")), notify=volumeChanged)
