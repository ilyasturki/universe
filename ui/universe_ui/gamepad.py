"""SDL2 game controllers → the keyboard events the theme already handles.

`Mapper` is pure state (testable without hardware); `GamepadThread` feeds it SDL events and
hands the transitions to the main thread, which posts QKeyEvents to the focused window.
Nothing is posted while a game holds the screen, since there is no focus window then.
"""

import logging
import time

from PySide6.QtCore import QCoreApplication, QEvent, QObject, QThread, QTimer, Qt, Signal, Slot
from PySide6.QtGui import QGuiApplication, QKeyEvent

log = logging.getLogger("universe.gamepad")

# SDL_GameControllerButton and SDL_GameControllerAxis values, so the mapper imports no SDL.
BTN_A, BTN_B, BTN_X, BTN_Y, BTN_BACK, BTN_GUIDE, BTN_START = 0, 1, 2, 3, 4, 5, 6
BTN_LEFTSTICK, BTN_RIGHTSTICK, BTN_LEFTSHOULDER, BTN_RIGHTSHOULDER = 7, 8, 9, 10
BTN_DPAD_UP, BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT = 11, 12, 13, 14
AXIS_LEFTX, AXIS_LEFTY, AXIS_RIGHTX, AXIS_RIGHTY, AXIS_TRIGGERLEFT, AXIS_TRIGGERRIGHT = 0, 1, 2, 3, 4, 5

BUTTON_KEYS = {
    BTN_A: Qt.Key.Key_Return,
    BTN_B: Qt.Key.Key_Escape,
    BTN_X: Qt.Key.Key_I,
    BTN_Y: Qt.Key.Key_F,
    BTN_LEFTSHOULDER: Qt.Key.Key_Q,
    BTN_RIGHTSHOULDER: Qt.Key.Key_E,
    BTN_START: Qt.Key.Key_F1,
    BTN_GUIDE: Qt.Key.Key_F1,
    BTN_DPAD_UP: Qt.Key.Key_Up,
    BTN_DPAD_DOWN: Qt.Key.Key_Down,
    BTN_DPAD_LEFT: Qt.Key.Key_Left,
    BTN_DPAD_RIGHT: Qt.Key.Key_Right,
}

# (negative key, positive key) per axis; triggers only go positive.
AXIS_KEYS = {
    AXIS_LEFTX: (Qt.Key.Key_Left, Qt.Key.Key_Right),
    AXIS_LEFTY: (Qt.Key.Key_Up, Qt.Key.Key_Down),
    AXIS_TRIGGERLEFT: (None, Qt.Key.Key_PageUp),
    AXIS_TRIGGERRIGHT: (None, Qt.Key.Key_PageDown),
}

REPEATING = {Qt.Key.Key_Up, Qt.Key.Key_Down, Qt.Key.Key_Left, Qt.Key.Key_Right}
AXIS_PRESS, AXIS_RELEASE = 0.5, 0.3
REPEAT_DELAY, REPEAT_INTERVAL = 0.35, 0.09


class Mapper:
    """Turns button/axis samples into (key, pressed, autorepeat) transitions."""

    def __init__(self, clock=time.monotonic):
        self._clock = clock
        self._axis = {}
        self._held = {}

    def button(self, button, pressed):
        key = BUTTON_KEYS.get(button)
        if key is None:
            return []
        return self._transition(key, pressed)

    def axis(self, axis, value):
        keys = AXIS_KEYS.get(axis)
        if keys is None:
            return []
        value = max(-1.0, min(1.0, value / 32767.0))
        current = self._axis.get(axis, 0)
        direction = current
        if abs(value) < AXIS_RELEASE:
            direction = 0
        elif abs(value) > AXIS_PRESS:
            direction = -1 if value < 0 else 1
        if direction == current:
            return []
        self._axis[axis] = direction
        out = []
        if current != 0 and keys[(current + 1) // 2] is not None:
            out += self._transition(keys[(current + 1) // 2], False)
        if direction != 0 and keys[(direction + 1) // 2] is not None:
            out += self._transition(keys[(direction + 1) // 2], True)
        return out

    def _transition(self, key, pressed):
        if pressed:
            if key in self._held:
                return []
            self._held[key] = self._clock() + REPEAT_DELAY
            return [(key, True, False)]
        if key not in self._held:
            return []
        del self._held[key]
        return [(key, False, False)]

    def tick(self):
        now = self._clock()
        out = []
        for key, due in self._held.items():
            if key in REPEATING and now >= due:
                self._held[key] = due + REPEAT_INTERVAL if now - due < REPEAT_INTERVAL else now + REPEAT_INTERVAL
                out.append((key, True, True))
        return out


def post_key(key, pressed, autorepeat=False, window=None):
    window = window or QGuiApplication.focusWindow()
    if window is None:
        return False
    kind = QEvent.Type.KeyPress if pressed else QEvent.Type.KeyRelease
    QCoreApplication.postEvent(window, QKeyEvent(kind, key, Qt.KeyboardModifier.NoModifier, "", autorepeat))
    return True


class GamepadThread(QThread):
    # QKeyEvent's constructor parents the primary QInputDevice to the app on first use, so events are built on the main thread.
    key = Signal(int, bool, bool)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._running = False
        self.mapper = Mapper()
        self.key.connect(self._post, Qt.ConnectionType.QueuedConnection)

    @Slot(int, bool, bool)
    def _post(self, key, pressed, autorepeat):
        post_key(Qt.Key(key), pressed, autorepeat)

    def stop(self):
        self._running = False
        self.wait(2000)

    def run(self):
        try:
            import sdl2
        except ImportError as e:
            log.warning("no gamepad support: %s", e)
            return
        sdl2.SDL_SetHint(sdl2.SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, b"1")
        if sdl2.SDL_Init(sdl2.SDL_INIT_GAMECONTROLLER | sdl2.SDL_INIT_JOYSTICK) != 0:
            log.warning("no gamepad support: %s", sdl2.SDL_GetError())
            return
        controllers = {}
        self._running = True
        event = sdl2.SDL_Event()
        try:
            while self._running:
                while sdl2.SDL_PollEvent(event):
                    self._handle(sdl2, event, controllers)
                for key, pressed, repeat in self.mapper.tick():
                    self.key.emit(key, pressed, repeat)
                sdl2.SDL_WaitEventTimeout(None, 20)
        finally:
            for c in controllers.values():
                sdl2.SDL_GameControllerClose(c)
            sdl2.SDL_Quit()

    def _handle(self, sdl2, event, controllers):
        t = event.type
        if t == sdl2.SDL_CONTROLLERDEVICEADDED:
            index = event.cdevice.which
            controller = sdl2.SDL_GameControllerOpen(index)
            if controller:
                joystick = sdl2.SDL_GameControllerGetJoystick(controller)
                controllers[sdl2.SDL_JoystickInstanceID(joystick)] = controller
                log.info("controller: %s", sdl2.SDL_GameControllerName(controller))
        elif t == sdl2.SDL_CONTROLLERDEVICEREMOVED:
            controller = controllers.pop(event.cdevice.which, None)
            if controller:
                sdl2.SDL_GameControllerClose(controller)
        elif t in (sdl2.SDL_CONTROLLERBUTTONDOWN, sdl2.SDL_CONTROLLERBUTTONUP):
            for key, pressed, repeat in self.mapper.button(event.cbutton.button, t == sdl2.SDL_CONTROLLERBUTTONDOWN):
                self.key.emit(key, pressed, repeat)
        elif t == sdl2.SDL_CONTROLLERAXISMOTION:
            for key, pressed, repeat in self.mapper.axis(event.caxis.axis, event.caxis.value):
                self.key.emit(key, pressed, repeat)


KEY_NAMES = {
    "Left": Qt.Key.Key_Left, "Right": Qt.Key.Key_Right, "Up": Qt.Key.Key_Up, "Down": Qt.Key.Key_Down,
    "Return": Qt.Key.Key_Return, "A": Qt.Key.Key_Return, "Esc": Qt.Key.Key_Escape, "B": Qt.Key.Key_Escape,
    "I": Qt.Key.Key_I, "X": Qt.Key.Key_I, "F": Qt.Key.Key_F, "Y": Qt.Key.Key_F,
    "Q": Qt.Key.Key_Q, "LB": Qt.Key.Key_Q, "E": Qt.Key.Key_E, "RB": Qt.Key.Key_E,
    "PgUp": Qt.Key.Key_PageUp, "LT": Qt.Key.Key_PageUp, "PgDown": Qt.Key.Key_PageDown, "RT": Qt.Key.Key_PageDown,
    "F1": Qt.Key.Key_F1, "Start": Qt.Key.Key_F1,
}


class KeyScript(QObject):
    """Posts a scripted key sequence, one name per gap: `Wait` idles, `Wait:N` idles N gaps,
    `Hold:A`/`Release:A` split a press, `Shot:path.png` grabs the window."""

    def __init__(self, script, gap_ms, window, parent=None):
        super().__init__(parent)
        self._queue = [k for k in script.split() if k]
        self._window = window
        self._timer = QTimer(self)
        self._timer.setInterval(gap_ms)
        self._timer.timeout.connect(self._step)

    def start(self, delay_ms):
        QTimer.singleShot(delay_ms, self._timer.start)

    def _step(self):
        if not self._queue:
            self._timer.stop()
            return
        name = self._queue.pop(0)
        phase, _, bare = name.partition(":")
        if phase == "Wait":
            if bare.isdigit() and int(bare) > 1:
                self._queue[0:0] = ["Wait"] * (int(bare) - 1)
            return
        if phase == "Shot":
            ok = self._window.grabWindow().save(bare)
            log.info("shot %s %s", "saved" if ok else "FAILED", bare)
            return
        if not bare:
            phase, bare = "click", name
        key = KEY_NAMES.get(bare)
        if key is None:
            log.warning("unknown key %s", name)
            return
        if phase in ("click", "Hold"):
            post_key(key, True, window=self._window)
        if phase in ("click", "Release"):
            post_key(key, False, window=self._window)
