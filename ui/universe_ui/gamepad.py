import ctypes
import functools
import logging
import os
import struct
import subprocess
import sys
import threading
import time

from PySide6.QtCore import QCoreApplication, QEvent, QObject, QSocketNotifier, Qt, QThread, QTimer, Signal, Slot
from PySide6.QtGui import QGuiApplication, QKeyEvent, QMouseEvent, QWheelEvent

log = logging.getLogger("universe.gamepad")

# SDL_GamepadButton and SDL_GamepadAxis values: positional, SOUTH first.
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
    BTN_DPAD_UP: Qt.Key.Key_Up,
    BTN_DPAD_DOWN: Qt.Key.Key_Down,
    BTN_DPAD_LEFT: Qt.Key.Key_Left,
    BTN_DPAD_RIGHT: Qt.Key.Key_Right,
}

AXIS_KEYS = {
    AXIS_LEFTX: {-1: Qt.Key.Key_Left, 1: Qt.Key.Key_Right},
    AXIS_LEFTY: {-1: Qt.Key.Key_Up, 1: Qt.Key.Key_Down},
    AXIS_RIGHTY: {-1: Qt.Key.Key_BracketLeft, 1: Qt.Key.Key_BracketRight},
    AXIS_TRIGGERLEFT: {1: Qt.Key.Key_PageUp},
    AXIS_TRIGGERRIGHT: {1: Qt.Key.Key_PageDown},
}

STICKS = {AXIS_RIGHTX: "rightX"}

# SDL's name for each slot of the watcher's device line; the extras have none the launcher reads.
SDL_SLOTS = {
    "lb": "leftshoulder",
    "rb": "rightshoulder",
    "lt": "lefttrigger",
    "rt": "righttrigger",
    "select": "back",
    "start": "start",
    "guide": "guide",
    "ls": "leftstick",
    "rs": "rightstick",
    "dpad_up": "dpup",
    "dpad_down": "dpdown",
    "dpad_left": "dpleft",
    "dpad_right": "dpright",
}
SDL_AXES = {"lx": "leftx", "ly": "lefty", "rx": "rightx", "ry": "righty", "lt": "lefttrigger", "rt": "righttrigger"}
FACE_POSITIONS = {"south": "a", "east": "b", "west": "x", "north": "y"}


# The SDL mapping fields for a pad the watcher described: `sdl` names each bound slot and `axes` each stick or trigger as SDL numbers the joystick,
# `labels` a slot's printed name. A lettered button goes to its letter, so the A of a Nintendo-style pad confirms wherever it sits; the rest go by
# position, and a trigger by its pull when it has one.
def mapping_fields(sdl, axes, labels):
    fields = {}
    lettered = {labels.get(slot): slot for slot in FACE_POSITIONS if labels.get(slot) in ("A", "B", "X", "Y")}
    for slot, name in FACE_POSITIONS.items():
        source = lettered.get(name.upper(), slot) if lettered else slot
        if sdl.get(source):
            fields[name] = sdl[source]
    for slot, name in SDL_SLOTS.items():
        if sdl.get(slot):
            fields[name] = sdl[slot]
    for role, name in SDL_AXES.items():
        if axes.get(role):
            fields[name] = axes[role]
    return ",".join(f"{name}:{element}" for name, element in fields.items())


INIT_GAMEPAD = 0x2000
EVENT_GAMEPAD_AXIS_MOTION, EVENT_GAMEPAD_BUTTON_DOWN, EVENT_GAMEPAD_BUTTON_UP, EVENT_GAMEPAD_ADDED, EVENT_GAMEPAD_REMOVED = 0x650, 0x651, 0x652, 0x653, 0x654
EVENT_SIZE = 128
# SDL_Event's type, its gamepad events' joystick id and button or axis, then an axis event's value.
EVENT_HEAD, EVENT_VALUE = struct.Struct("<I12xIB"), struct.Struct("<24xh")


class _GUID(ctypes.Structure):
    _fields_ = [("data", ctypes.c_uint8 * 16)]


_P = ctypes.c_void_p
_SIGNATURES = {
    "SDL_SetHint": (ctypes.c_bool, [ctypes.c_char_p, ctypes.c_char_p]),
    "SDL_Init": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_Quit": (None, []),
    "SDL_InitSubSystem": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_QuitSubSystem": (None, [ctypes.c_uint32]),
    "SDL_GetError": (ctypes.c_char_p, []),
    "SDL_PollEvent": (ctypes.c_bool, [_P]),
    "SDL_WaitEventTimeout": (ctypes.c_bool, [_P, ctypes.c_int32]),
    "SDL_GetJoystickGUIDForID": (_GUID, [ctypes.c_uint32]),
    "SDL_GetJoystickNameForID": (ctypes.c_char_p, [ctypes.c_uint32]),
    "SDL_GetJoystickVendorForID": (ctypes.c_uint16, [ctypes.c_uint32]),
    "SDL_GetJoystickProductForID": (ctypes.c_uint16, [ctypes.c_uint32]),
    "SDL_AddGamepadMapping": (ctypes.c_int, [ctypes.c_char_p]),
    "SDL_OpenGamepad": (_P, [ctypes.c_uint32]),
    "SDL_CloseGamepad": (None, [_P]),
    "SDL_GetGamepadName": (ctypes.c_char_p, [_P]),
}


def load_sdl():
    lib = ctypes.CDLL("libSDL3.so.0")
    for name, (restype, argtypes) in _SIGNATURES.items():
        fn = getattr(lib, name)
        fn.restype = restype
        fn.argtypes = argtypes
    return lib


STICK_DEADZONE = 0.18

REPEATING = {Qt.Key.Key_Up, Qt.Key.Key_Down, Qt.Key.Key_Left, Qt.Key.Key_Right, Qt.Key.Key_BracketLeft, Qt.Key.Key_BracketRight}
AXIS_PRESS, AXIS_RELEASE = 0.5, 0.3
REPEAT_DELAY, REPEAT_INTERVAL = 0.35, 0.09


class Mapper:
    def __init__(self, clock=time.monotonic):
        self._clock = clock
        self._axis = {}
        self.held = {}
        self._stick = {}

    def button(self, button, pressed):
        key = BUTTON_KEYS.get(button)
        return [] if key is None else self._transition(key, pressed)

    def axis(self, axis, value):
        keys = AXIS_KEYS.get(axis)
        if keys is None:
            return []
        value /= 32767.0
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
        if keys.get(current) is not None:
            out += self._transition(keys[current], False)
        if keys.get(direction) is not None:
            out += self._transition(keys[direction], True)
        return out

    def stick(self, axis, value):
        name = STICKS.get(axis)
        if name is None:
            return None
        value = max(-1.0, min(1.0, value / 32767.0))
        magnitude = abs(value)
        value = 0.0 if magnitude < STICK_DEADZONE else (magnitude - STICK_DEADZONE) / (1.0 - STICK_DEADZONE) * (1 if value > 0 else -1)
        if self._stick.get(name, 0.0) == value:
            return None
        self._stick[name] = value
        return name, value

    def _transition(self, key, pressed):
        if pressed:
            if key in self.held:
                return []
            self.held[key] = self._clock() + REPEAT_DELAY
            return [(key, True, False)]
        if key not in self.held:
            return []
        del self.held[key]
        return [(key, False, False)]

    # Every held key released and the axes forgotten: what the game presses from here is not ours.
    def release_all(self):
        out = [(key, False, False) for key in self.held]
        self.held.clear()
        self._axis.clear()
        return out

    # Each stick off centre set back to it, so a scrub held as the pad is taken away does not run on.
    def center_sticks(self):
        out = [(name, 0.0) for name, value in self._stick.items() if value]
        self._stick.clear()
        return out

    def tick(self):
        now = self._clock()
        out = []
        for key, due in self.held.items():
            if key in REPEATING and now >= due:
                self.held[key] = due + REPEAT_INTERVAL if now - due < REPEAT_INTERVAL else now + REPEAT_INTERVAL
                out.append((key, True, True))
        return out


# The keys posted here, keyed by (key, pressed), each with who posted it: the window's filter tells them from the keyboard's own.
POSTED = {}


def post_key(key, pressed, autorepeat=False, window=None, source="pad"):
    window = window or QGuiApplication.focusWindow()
    if window is None:
        return False
    kind = QEvent.Type.KeyPress if pressed else QEvent.Type.KeyRelease
    POSTED.setdefault((int(key), pressed), []).append(source)
    QCoreApplication.postEvent(window, QKeyEvent(kind, key, Qt.KeyboardModifier.NoModifier, "", autorepeat))
    return True


def posted_source(key, pressed):
    sources = POSTED.get((int(key), pressed))
    if not sources:
        return None
    source = sources.pop(0)
    if not sources:
        del POSTED[(int(key), pressed)]
    return source


class GamepadThread(QThread):
    # QKeyEvent's constructor parents the primary QInputDevice to the app on first use, so events are built on the main thread.
    key = Signal(int, bool, bool)
    stick = Signal(str, float)

    def __init__(self, parent=None, pad=None):
        super().__init__(parent)
        self._covered = False
        self._pad = pad
        self._swallowed = set()
        self.mapper = Mapper()
        self._mappings = {}
        self._pending = []
        self._rescan = False
        self._lock = threading.Lock()
        self.key.connect(self._post, Qt.ConnectionType.QueuedConnection)

    # Find controllers: the loop lets SDL go and starts it again, each pad found added afresh with its mapping.
    @Slot()
    def rescan(self):
        self._rescan = True

    # A mapping from the watcher's reading of the pad, applied by the loop: SDL's tables are edited from its own thread.
    @Slot(int, int, str)
    def setMapping(self, vendor, product, fields):
        if not fields:
            return
        with self._lock:
            self._mappings[(int(vendor), int(product))] = fields
            self._pending.append((int(vendor), int(product)))

    def _apply_mappings(self, sdl, controllers):
        with self._lock:
            pending, self._pending = self._pending, []
        for vendor, product in pending:
            fields = self._mappings.get((vendor, product))
            for which in controllers:
                if (sdl.SDL_GetJoystickVendorForID(which), sdl.SDL_GetJoystickProductForID(which)) == (vendor, product):
                    self._add_mapping(sdl, which, fields)

    def _add_mapping(self, sdl, which, fields):
        guid = bytes(sdl.SDL_GetJoystickGUIDForID(which).data).hex()
        name = (sdl.SDL_GetJoystickNameForID(which) or b"pad").decode(errors="replace").replace(",", " ")
        line = f"{guid},{name},{fields},"
        if sdl.SDL_AddGamepadMapping(line.encode()) < 0:
            log.warning("mapping refused: %s", sdl.SDL_GetError())
        else:
            log.info("mapping: %s", line)

    # Muted or covered, a press is dropped with its release: a release on its own steps a section or answers a dialog.
    @Slot(int, bool, bool)
    def _post(self, key, pressed, autorepeat):
        if pressed and (self._covered or (self._pad is not None and self._pad.muted)):
            if not autorepeat:
                self._swallowed.add(key)
            if self._covered and os.environ.get("UNIVERSE_UI_INPUT_LOG"):
                log.info("pad key %d dropped: covered", key)
            return
        if not pressed and key in self._swallowed:
            self._swallowed.discard(key)
            return
        post_key(Qt.Key(key), pressed, autorepeat)

    # The game is on screen, or another app has the focus: the presses are not read, and the loop sleeps between hot-plug checks.
    def setCovered(self, covered):
        self._covered = bool(covered)

    def stop(self):
        self.requestInterruption()
        self.wait()

    def run(self):
        try:
            sdl = load_sdl()
        except (OSError, AttributeError) as e:
            log.warning("no gamepad support: %s", e)
            return
        sdl.SDL_SetHint(b"SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", b"1")
        # The pads are read through evdev, as the watcher reads them: its mapping then names the same buttons.
        sdl.SDL_SetHint(b"SDL_JOYSTICK_HIDAPI", b"0")
        if not sdl.SDL_Init(INIT_GAMEPAD):
            log.warning("no gamepad support: %s", sdl.SDL_GetError())
            return
        controllers = {}
        event = ctypes.create_string_buffer(EVENT_SIZE)
        try:
            while not self.isInterruptionRequested():
                if self._rescan:
                    self._rescan = False
                    self._restart(sdl, controllers)
                while sdl.SDL_PollEvent(event):
                    self._handle(sdl, event, controllers)
                if self._pending:
                    self._apply_mappings(sdl, controllers)
                if self._covered:
                    for name, value in self.mapper.center_sticks():
                        self.stick.emit(name, value)
                    ticks = self.mapper.release_all()
                else:
                    ticks = self.mapper.tick()
                for key, pressed, repeat in ticks:
                    self.key.emit(key, pressed, repeat)
                sdl.SDL_WaitEventTimeout(None, 20 if self.mapper.held and not self._covered else 500)
        finally:
            for c in controllers.values():
                sdl.SDL_CloseGamepad(c)
            sdl.SDL_Quit()

    def _restart(self, sdl, controllers):
        for c in controllers.values():
            sdl.SDL_CloseGamepad(c)
        controllers.clear()
        for key, pressed, repeat in self.mapper.release_all():
            self.key.emit(key, pressed, repeat)
        sdl.SDL_QuitSubSystem(INIT_GAMEPAD)
        if not sdl.SDL_InitSubSystem(INIT_GAMEPAD):
            log.warning("gamepad restart: %s", sdl.SDL_GetError())

    def _handle(self, sdl, event, controllers):
        t, which, part = EVENT_HEAD.unpack_from(event)
        if t == EVENT_GAMEPAD_ADDED:
            fields = self._mappings.get((sdl.SDL_GetJoystickVendorForID(which), sdl.SDL_GetJoystickProductForID(which)))
            if fields:
                self._add_mapping(sdl, which, fields)
            controller = sdl.SDL_OpenGamepad(which)
            if controller:
                controllers[which] = controller
                log.info("controller: %s", (sdl.SDL_GetGamepadName(controller) or b"?").decode(errors="replace"))
        elif t == EVENT_GAMEPAD_REMOVED:
            controller = controllers.pop(which, None)
            if controller:
                sdl.SDL_CloseGamepad(controller)
        elif self._covered:
            return
        elif t in (EVENT_GAMEPAD_BUTTON_DOWN, EVENT_GAMEPAD_BUTTON_UP):
            for key, pressed, repeat in self.mapper.button(part, t == EVENT_GAMEPAD_BUTTON_DOWN):
                self.key.emit(key, pressed, repeat)
        elif t == EVENT_GAMEPAD_AXIS_MOTION:
            (value,) = EVENT_VALUE.unpack_from(event)
            for key, pressed, repeat in self.mapper.axis(part, value):
                self.key.emit(key, pressed, repeat)
            moved = self.mapper.stick(part, value)
            if moved is not None:
                self.stick.emit(*moved)


KEY_NAMES = {
    "Left": Qt.Key.Key_Left,
    "Right": Qt.Key.Key_Right,
    "Up": Qt.Key.Key_Up,
    "Down": Qt.Key.Key_Down,
    "Return": Qt.Key.Key_Return,
    "A": Qt.Key.Key_Return,
    "Esc": Qt.Key.Key_Escape,
    "B": Qt.Key.Key_Escape,
    "I": Qt.Key.Key_I,
    "X": Qt.Key.Key_I,
    "F": Qt.Key.Key_F,
    "Y": Qt.Key.Key_F,
    "Q": Qt.Key.Key_Q,
    "LB": Qt.Key.Key_Q,
    "E": Qt.Key.Key_E,
    "RB": Qt.Key.Key_E,
    "PgUp": Qt.Key.Key_PageUp,
    "LT": Qt.Key.Key_PageUp,
    "PgDown": Qt.Key.Key_PageDown,
    "RT": Qt.Key.Key_PageDown,
    "F1": Qt.Key.Key_F1,
    "Start": Qt.Key.Key_F1,
    "Backspace": Qt.Key.Key_Backspace,
    "Home": Qt.Key.Key_Home,
    "End": Qt.Key.Key_End,
    "BracketLeft": Qt.Key.Key_BracketLeft,
    "RSUp": Qt.Key.Key_BracketLeft,
    "BracketRight": Qt.Key.Key_BracketRight,
    "RSDown": Qt.Key.Key_BracketRight,
}


def post_mouse(window, kind, x, y, button=Qt.MouseButton.NoButton):
    from PySide6.QtCore import QPointF

    pos = QPointF(x, y)
    held = button if kind == QEvent.Type.MouseButtonPress else Qt.MouseButton.NoButton
    QCoreApplication.postEvent(window, QMouseEvent(kind, pos, pos, window.mapToGlobal(pos.toPoint()), button, held, Qt.KeyboardModifier.NoModifier))


@functools.cache
def _touch_device():
    from PySide6.QtTest import QTest

    return QTest.createTouchDevice()


def touch(window, points, hold_ms=0):
    """One finger down at the first point, through the others, up at the last: a tap, a press held `hold_ms`, or a swipe."""
    from PySide6.QtCore import QPoint
    from PySide6.QtTest import QTest

    device = _touch_device()
    (x, y), *rest = points
    QTest.touchEvent(window, device).press(0, QPoint(round(x), round(y)), window).commit()
    QTest.qWait(hold_ms)
    for x, y in rest:
        QTest.qWait(16)
        QTest.touchEvent(window, device).move(0, QPoint(round(x), round(y)), window).commit()
    x, y = points[-1]
    QTest.touchEvent(window, device).release(0, QPoint(round(x), round(y)), window).commit()


def post_wheel(window, x, y, steps, sideways=False, pixels=False):
    from PySide6.QtCore import QPoint, QPointF

    pos = QPointF(x, y)
    delta = QPoint(-steps, 0) if sideways else QPoint(0, steps)
    if pixels:
        delta, pixel = QPoint(), delta
    else:
        delta, pixel = delta * 120, QPoint()
    QCoreApplication.postEvent(
        window,
        QWheelEvent(
            pos,
            window.mapToGlobal(pos.toPoint()),
            pixel,
            delta,
            Qt.MouseButton.NoButton,
            Qt.KeyboardModifier.NoModifier,
            Qt.ScrollPhase.NoScrollPhase,
            False,
        ),
    )


EXPECT_FAILED = 3


# The id of the game the Universe look's Return would launch, "" for none, as in HOME's dock; None, which no Expect matches, on another look, window or dialog.
def launch_target(window):
    focus = QGuiApplication.focusWindow()
    if focus not in (None, window):
        return "" if focus.objectName() == "homeOverlay" else None
    look = window.findChild(QObject, "look")
    root = look.property("item") if look is not None else None
    return root.property("returnLaunches") if root is not None else None


# `--keys`, one name per gap: `Wait`, `Wait:N`, `Hold:A`/`Release:A`, `Stick:rightX=0.6`, `Shot:path.png`, `Guide`; `Pad:A`/`PadHold:A`/`PadRelease:A` through the
# pad thread, dropped as its presses are while covered; as if the watcher read them, `Press:slot`/`Unpress:slot`, `Axis:lx=0.6`;
# the mouse, at 1080p design units (dp, so 1728 wide at 16:10): `Mouse:x,y` moves it, `Click:x,y` / `RightClick:x,y` press and release there, `MouseDown:x,y` / `MouseUp:x,y` one or the other,
# `Wheel:x,y,N` rolls N notches (up positive), `HWheel:x,y,N` sideways (right positive), `Scroll:x,y,N` N pixels as a touchpad; `Type:text` types it from the keyboard (`_` a space);
# a finger: `Tap:x,y`, `LongTap:x,y,ms` held that long, `Swipe:x1,y1,x2,y2` dragged across in eight moves; `Await` holds the script until a line comes on stdin,
# `Ready` until a game is on screen for Return to launch, `Quit` quits;
# `Volume:up`/`Volume:down`/`Volume:mute` as a pad's volume macro; `Expect:ID` quits with EXPECT_FAILED unless Return would launch game ID (`Expect:` none);
# `Mark:NAME` logs NAME with the wall-clock time, which tools/ui-record turns into a time in its recording; `Reset` reloads the look, back on its first screen;
# `Game:KEY` presses an X keysym in the game on $DISPLAY through xdotool (a gamescope take's Xwayland), `GameHold:KEY`/`GameRelease:KEY` one half.
# A game that polls its keys once a frame misses a press released within the same frame: Game: holds it 200 ms.
GAME_KEYS = {"Game": ("keydown", "{}", "sleep", "0.2", "keyup", "{}"), "GameHold": ("keydown", "{}"), "GameRelease": ("keyup", "{}")}


class KeyScript(QObject):
    def __init__(self, script, gap_ms, window, pad=None, watcher=None, home=None, gamepad=None, parent=None):
        super().__init__(parent)
        self._queue = [k for k in script.split() if k]
        self._window = window
        self._pad = pad
        self._gamepad = gamepad
        self._watcher = watcher
        self._home = home
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
        if phase == "Await":
            self._timer.stop()
            stdin = QSocketNotifier(sys.stdin.fileno(), QSocketNotifier.Type.Read, self)

            def resume():
                sys.stdin.readline()
                stdin.setEnabled(False)
                stdin.deleteLater()
                self._timer.start()

            stdin.activated.connect(resume)
            return
        if phase == "Quit":
            QCoreApplication.quit()
            return
        if phase == "Ready":
            if not launch_target(self._window):
                self._queue.insert(0, name)
            return
        if phase == "Expect":
            target = launch_target(self._window)
            if target != bare:
                log.error("expected Return to launch %s, not %s: quitting", bare or "nothing", "unknown" if target is None else target or "nothing")
                self._queue.clear()
                QCoreApplication.exit(EXPECT_FAILED)
            return
        if phase == "Volume":
            if bare not in ("up", "down", "mute"):
                log.warning("unknown key %s", name)
            elif self._home is not None:
                self._home.volumeMacro(bare)
            return
        if phase == "Mark":
            log.info("mark %s %.3f", bare, time.time())
            return
        if phase in GAME_KEYS:
            try:
                subprocess.Popen(["xdotool", *(step.format(bare) for step in GAME_KEYS[phase])])
            except OSError as e:
                log.warning("%s: %s", name, e)
            return
        if phase == "Reset":
            look = self._window.findChild(QObject, "look")
            look.setProperty("active", False)
            look.setProperty("active", True)
            return
        if phase == "Shot":
            ok = self._window.grabWindow().save(bare)
            log.info("shot %s %s", "saved" if ok else "FAILED", bare)
            return
        if phase == "Stick":
            axis, _, value = bare.partition("=")
            if self._pad is not None:
                self._pad.set(axis, float(value or 0))
            return
        if phase in ("Pad", "PadHold", "PadRelease"):
            key = KEY_NAMES.get(bare)
            if self._gamepad is not None and key is not None:
                if phase in ("Pad", "PadHold"):
                    self._gamepad.key.emit(int(key), True, False)
                if phase in ("Pad", "PadRelease"):
                    self._gamepad.key.emit(int(key), False, False)
            return
        if phase in ("Press", "Unpress"):
            if self._watcher is not None:
                self._watcher.press(bare, phase == "Press")
            return
        if phase in ("Tap", "LongTap", "Swipe"):
            parts = [float(v) * self._window.height() / 1080 for v in bare.split(",")]
            if phase == "Swipe":
                (x1, y1, x2, y2) = parts[:4]
                touch(self._window, [(x1 + (x2 - x1) * k / 8, y1 + (y2 - y1) * k / 8) for k in range(9)])
            else:
                hold = int(float(bare.split(",")[2])) if phase == "LongTap" else 0
                touch(self._window, [(parts[0], parts[1])], hold)
            return
        if phase in ("Mouse", "Click", "RightClick", "MouseDown", "MouseUp", "Wheel", "HWheel", "Scroll"):
            parts = [float(v) for v in bare.split(",")]
            scale = self._window.height() / 1080
            x, y = parts[0] * scale, parts[1] * scale
            if phase in ("Wheel", "HWheel", "Scroll"):
                post_wheel(self._window, x, y, int(parts[2]) if len(parts) > 2 else 1, phase == "HWheel", phase == "Scroll")
                return
            post_mouse(self._window, QEvent.Type.MouseMove, x, y)
            button = Qt.MouseButton.RightButton if phase == "RightClick" else Qt.MouseButton.LeftButton
            if phase in ("Click", "RightClick", "MouseDown"):
                post_mouse(self._window, QEvent.Type.MouseButtonPress, x, y, button)
            if phase in ("Click", "RightClick", "MouseUp"):
                post_mouse(self._window, QEvent.Type.MouseButtonRelease, x, y, button)
            return
        if phase == "Type":
            window = QGuiApplication.focusWindow() or self._window
            for ch in bare.replace("_", " "):
                key = Qt.Key(ord(ch.upper())) if ch.isalnum() or ch == " " else Qt.Key.Key_unknown
                for kind in (QEvent.Type.KeyPress, QEvent.Type.KeyRelease):
                    QCoreApplication.postEvent(window, QKeyEvent(kind, key, Qt.KeyboardModifier.NoModifier, ch))
            return
        if phase == "Axis":
            axis, _, value = bare.partition("=")
            if self._watcher is not None:
                self._watcher.axis(axis, float(value or 0))
            return
        if not bare:
            phase, bare = "click", name
        if bare == "Guide":
            if self._home is not None:
                if phase in ("click", "Hold"):
                    self._home.guide(True)
                if phase in ("click", "Release"):
                    self._home.guide(False)
            return
        key = KEY_NAMES.get(bare)
        if key is None:
            log.warning("unknown key %s", name)
            return
        window = QGuiApplication.focusWindow() or self._window
        if phase in ("click", "Hold"):
            post_key(key, True, window=window, source="script")
        if phase in ("click", "Release"):
            post_key(key, False, window=window, source="script")
