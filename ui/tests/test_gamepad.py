from types import SimpleNamespace

import pytest
from PySide6.QtCore import QCoreApplication, Qt
from uitest import record, until

from universe_ui import gamepad
from universe_ui.gamepad import Mapper


class Clock:
    def __init__(self):
        self.now = 100.0

    def __call__(self):
        return self.now


def test_buttons_map_to_theme_keys():
    m = Mapper(Clock())
    assert m.button(gamepad.BTN_A, True) == [(Qt.Key.Key_Return, True, False)]
    assert m.button(gamepad.BTN_A, True) == []
    assert m.button(gamepad.BTN_A, False) == [(Qt.Key.Key_Return, False, False)]
    assert m.button(gamepad.BTN_A, False) == []
    assert m.button(gamepad.BTN_BACK, True) == []


def test_axis_hysteresis():
    m = Mapper(Clock())
    assert m.axis(gamepad.AXIS_LEFTX, 32767) == [(Qt.Key.Key_Right, True, False)]
    assert m.axis(gamepad.AXIS_LEFTX, 12000) == []
    assert m.axis(gamepad.AXIS_LEFTX, 5000) == [(Qt.Key.Key_Right, False, False)]
    assert m.axis(gamepad.AXIS_LEFTX, -32767) == [(Qt.Key.Key_Left, True, False)]
    assert m.axis(gamepad.AXIS_LEFTX, 32767) == [(Qt.Key.Key_Left, False, False), (Qt.Key.Key_Right, True, False)]
    assert m.axis(gamepad.AXIS_LEFTY, -30000) == [(Qt.Key.Key_Up, True, False)]
    assert m.axis(gamepad.AXIS_TRIGGERRIGHT, 30000) == [(Qt.Key.Key_PageDown, True, False)]
    assert m.axis(gamepad.AXIS_TRIGGERLEFT, 30000) == [(Qt.Key.Key_PageUp, True, False)]
    assert m.axis(gamepad.AXIS_TRIGGERRIGHT, 0) == [(Qt.Key.Key_PageDown, False, False)]
    assert m.axis(gamepad.AXIS_RIGHTX, 32767) == []


def test_right_stick_vertical_pages_and_repeats():
    clock = Clock()
    m = Mapper(clock)
    assert m.axis(gamepad.AXIS_RIGHTY, 30000) == [(Qt.Key.Key_BracketRight, True, False)]
    assert m.axis(gamepad.AXIS_RIGHTY, 12000) == []
    clock.now += gamepad.REPEAT_DELAY + 0.01
    assert m.tick() == [(Qt.Key.Key_BracketRight, True, True)]
    assert m.axis(gamepad.AXIS_RIGHTY, 0) == [(Qt.Key.Key_BracketRight, False, False)]
    assert m.axis(gamepad.AXIS_RIGHTY, -30000) == [(Qt.Key.Key_BracketLeft, True, False)]
    assert m.stick(gamepad.AXIS_RIGHTY, -30000) is None


def test_right_stick_is_analog_past_the_deadzone():
    m = Mapper(Clock())
    assert m.stick(gamepad.AXIS_LEFTX, 32767) is None
    assert m.stick(gamepad.AXIS_RIGHTX, 3000) is None
    assert m.stick(gamepad.AXIS_RIGHTX, 32767) == ("rightX", 1.0)
    assert m.stick(gamepad.AXIS_RIGHTX, 32767) is None
    name, value = m.stick(gamepad.AXIS_RIGHTX, -16384)
    assert name == "rightX" and -0.40 < value < -0.38
    assert m.stick(gamepad.AXIS_RIGHTX, 1000) == ("rightX", 0.0)
    assert m.axis(gamepad.AXIS_RIGHTX, 32767) == []


def test_autorepeat_only_for_arrows():
    clock = Clock()
    m = Mapper(clock)
    m.button(gamepad.BTN_DPAD_RIGHT, True)
    m.button(gamepad.BTN_A, True)
    assert m.tick() == []
    clock.now += gamepad.REPEAT_DELAY + 0.01
    assert m.tick() == [(Qt.Key.Key_Right, True, True)]
    assert m.tick() == []
    clock.now += gamepad.REPEAT_INTERVAL + 0.01
    assert m.tick() == [(Qt.Key.Key_Right, True, True)]
    m.button(gamepad.BTN_DPAD_RIGHT, False)
    clock.now += 1
    assert m.tick() == []


# The Pro 3 in D-input mode, as the watcher lines it: its A on the right is BTN_SOUTH, b0 to SDL.
PRO3_SDL = {
    "south": "b1", "east": "b0", "north": "b3", "west": "b4", "lb": "b6", "rb": "b7", "lt": "b8", "rt": "b9",
    "select": "b10", "start": "b11", "guide": "b12", "ls": "b13", "rs": "b14",
    "dpad_up": "h0.1", "dpad_down": "h0.4", "dpad_left": "h0.8", "dpad_right": "h0.2",
    "paddle_l4": "b16", "paddle_r4": "b17", "paddle_pl": "b5", "paddle_pr": "b2",
}  # fmt: skip
PRO3_AXES = {"lx": "a0", "ly": "a1", "rx": "a2", "ry": "a3", "lt": "a5", "rt": "a4"}
PRO3_LABELS = {"south": "B", "east": "A", "north": "X", "west": "Y", "lb": "L1", "rb": "R1", "lt": "L2", "rt": "R2", "guide": "Home"}


def test_mapping_fields_follow_the_letters_and_the_positions():
    fields = dict(f.split(":") for f in gamepad.mapping_fields(PRO3_SDL, PRO3_AXES, PRO3_LABELS).split(","))
    assert (fields["a"], fields["b"], fields["x"], fields["y"]) == ("b0", "b1", "b3", "b4"), "the printed A confirms wherever it sits"
    assert (fields["lefttrigger"], fields["righttrigger"]) == ("a5", "a4"), "a trigger's pull is read before its click"
    assert (fields["rightx"], fields["righty"], fields["dpup"], fields["dpleft"], fields["back"], fields["guide"]) == ("a2", "a3", "h0.1", "h0.8", "b10", "b12")
    assert "paddle1" not in fields and "b16" not in fields.values(), "the launcher reads no extra"
    sony = {"south": "Cross", "east": "Circle", "north": "Triangle", "west": "Square"}
    positional = dict(f.split(":") for f in gamepad.mapping_fields(PRO3_SDL, PRO3_AXES, sony).split(","))
    assert (positional["a"], positional["b"], positional["x"], positional["y"]) == ("b1", "b0", "b4", "b3")
    digital = {"south": "b0", "east": "b1", "lt": "b8"}
    assert gamepad.mapping_fields(digital, {"lx": "a0"}, {}) == "a:b0,b:b1,lefttrigger:b8,leftx:a0"
    assert gamepad.mapping_fields({}, {}, {}) == ""


def test_covering_releases_what_was_held():
    m = Mapper(Clock())
    m.button(gamepad.BTN_A, True)
    m.axis(gamepad.AXIS_LEFTX, 32767)
    assert m.release_all() == [(Qt.Key.Key_Return, False, False), (Qt.Key.Key_Right, False, False)]
    assert m.held == {} and m.release_all() == []
    assert m.axis(gamepad.AXIS_LEFTX, 32767) == [(Qt.Key.Key_Right, True, False)], "the axis is forgotten: the next push presses again"


def test_covering_centers_a_stick_held_off_centre():
    m = Mapper(Clock())
    m.stick(gamepad.AXIS_RIGHTX, 32767)
    assert m.center_sticks() == [("rightX", 0.0)], "a scrub held as the pad is taken away stops"
    assert m.center_sticks() == []
    assert m.stick(gamepad.AXIS_RIGHTX, 32767) == ("rightX", 1.0)


# Another app took the focus: a press is dropped with its release, the release of a key pressed before still goes out.
def test_a_covered_pad_drops_a_press_with_its_release(app, monkeypatch):
    posted = []
    monkeypatch.setattr(gamepad, "post_key", lambda key, pressed, autorepeat=False, window=None: posted.append((key, pressed)))
    thread = gamepad.GamepadThread()
    thread._post(int(Qt.Key.Key_Return), True, False)
    thread.setCovered(True)
    thread._post(int(Qt.Key.Key_Escape), True, False)
    thread._post(int(Qt.Key.Key_Return), False, False)
    thread._post(int(Qt.Key.Key_Down), True, True)
    thread.setCovered(False)
    thread._post(int(Qt.Key.Key_Escape), False, False)
    thread._post(int(Qt.Key.Key_Down), False, False)
    assert posted == [(Qt.Key.Key_Return, True), (Qt.Key.Key_Return, False), (Qt.Key.Key_Down, False)], "a dropped repeat does not take its key's release"


@pytest.fixture
def pad(app, monkeypatch):
    sdl2 = pytest.importorskip("sdl2")
    posted = []
    monkeypatch.setattr(gamepad, "post_key", lambda key, pressed, autorepeat=False, window=None: posted.append((int(key), pressed)))
    thread = gamepad.GamepadThread()
    sdl = SimpleNamespace(mappings=[], closed=[], **{name: getattr(sdl2, name) for name in dir(sdl2) if name.startswith("SDL_CONTROLLER")})
    sdl.__dict__.update(
        SDL_JoystickGetDeviceVendor=lambda index: 0x0F0D,
        SDL_JoystickGetDeviceProduct=lambda index: 0x0186,
        SDL_JoystickGetDeviceGUID=lambda index: b"03000000f0d0000086010000",
        SDL_JoystickGetGUIDString=lambda guid, buf, size: setattr(buf, "value", guid),
        SDL_JoystickNameForIndex=lambda index: b"Pro 3, D-input",
        SDL_GameControllerAddMapping=lambda line: sdl.mappings.append(line.decode()) or 1,
        SDL_GameControllerOpen=lambda index: f"pad{index}",
        SDL_GameControllerGetJoystick=lambda controller: controller,
        SDL_JoystickInstanceID=lambda joystick: 7,
        SDL_GameControllerName=lambda controller: b"Pro 3",
        SDL_GameControllerClose=sdl.closed.append,
    )
    controllers, moved = {}, record(thread.stick)

    def feed(*specs):
        for kind, *fields in specs:
            event = sdl2.SDL_Event()
            if kind in ("down", "up"):
                event.type = sdl.SDL_CONTROLLERBUTTONDOWN if kind == "down" else sdl.SDL_CONTROLLERBUTTONUP
                event.cbutton.button = fields[0]
            elif kind == "axis":
                event.type = sdl.SDL_CONTROLLERAXISMOTION
                event.caxis.axis, event.caxis.value = fields
            else:
                event.type = sdl.SDL_CONTROLLERDEVICEADDED if kind == "added" else sdl.SDL_CONTROLLERDEVICEREMOVED
                event.cdevice.which = fields[0]
            thread._handle(sdl, event, controllers)
        QCoreApplication.sendPostedEvents(thread)

    return SimpleNamespace(thread=thread, sdl=sdl, controllers=controllers, posted=posted, moved=moved, feed=feed)


RETURN, DOWN, PAGE_UP = int(Qt.Key.Key_Return), int(Qt.Key.Key_Down), int(Qt.Key.Key_PageUp)


@pytest.mark.parametrize(
    ("events", "posted", "moved"),
    [
        ([("down", gamepad.BTN_A)], [(RETURN, True)], []),
        ([("down", gamepad.BTN_A), ("up", gamepad.BTN_A)], [(RETURN, True), (RETURN, False)], []),
        ([("down", gamepad.BTN_BACK)], [], []),
        ([("axis", gamepad.AXIS_LEFTY, 32767), ("axis", gamepad.AXIS_LEFTY, 0)], [(DOWN, True), (DOWN, False)], []),
        ([("axis", gamepad.AXIS_TRIGGERLEFT, 30000)], [(PAGE_UP, True)], []),
        ([("axis", gamepad.AXIS_RIGHTX, -32767)], [], [("rightX", -1.0)]),
    ],
    ids=["press", "press-release", "unmapped", "stick-push-and-back", "trigger", "right-stick-scrubs"],
)
def test_a_pad_event_posts_its_key(pad, events, posted, moved):
    pad.feed(*events)
    assert (pad.posted, pad.moved) == (posted, moved)


def test_a_covered_pad_reads_no_input_but_tracks_its_devices(pad):
    pad.thread.setMapping(0x0F0D, 0x0186, "a:b1,b:b0")
    pad.thread.setCovered(True)
    pad.feed(("added", 0), ("down", gamepad.BTN_A), ("axis", gamepad.AXIS_RIGHTX, 32767))
    assert (pad.posted, pad.moved) == ([], [])
    assert pad.controllers == {7: "pad0"}
    assert pad.sdl.mappings == ["03000000f0d0000086010000,Pro 3  D-input,a:b1,b:b0,"], "a comma would end the name in SDL's line"
    pad.feed(("removed", 7))
    assert pad.controllers == {} and pad.sdl.closed == ["pad0"]
    pad.thread.setCovered(False)
    pad.feed(("down", gamepad.BTN_A))
    assert pad.posted == [(RETURN, True)]


def test_post_key_needs_a_focus_window(app):
    assert gamepad.post_key(Qt.Key.Key_Return, True) is False


# A press from before the mute still gets its release; a press dropped by it drops its release too, or a section would step on it.
def test_a_muted_pad_drops_a_press_with_its_release(app, monkeypatch):
    from universe_ui.api import Pad

    posted = []
    monkeypatch.setattr(gamepad, "post_key", lambda key, pressed, autorepeat=False, window=None: posted.append((key, pressed)))
    pad = Pad()
    thread = gamepad.GamepadThread(pad=pad)
    thread._post(int(Qt.Key.Key_Return), True, False)
    pad.muted = True
    thread._post(int(Qt.Key.Key_PageUp), True, False)
    thread._post(int(Qt.Key.Key_Return), False, False)
    pad.muted = False
    thread._post(int(Qt.Key.Key_PageUp), False, False)
    thread._post(int(Qt.Key.Key_PageUp), True, False)
    thread._post(int(Qt.Key.Key_PageUp), False, False)
    assert posted == [(Qt.Key.Key_Return, True), (Qt.Key.Key_Return, False), (Qt.Key.Key_PageUp, True), (Qt.Key.Key_PageUp, False)]


def test_key_script_plays_a_fake_pad(app):
    from universe_ui.screens.controller import FakeWatcher

    watcher = FakeWatcher("xbox")
    lines = []
    watcher.received.connect(lines.append)
    script = gamepad.KeyScript("Press:south Axis:lx=-0.5 Unpress:south", 1, None, watcher=watcher)
    for _ in range(3):
        script._step()
    assert [(line["event"], line.get("slot") or line.get("axis"), line.get("pressed", line.get("value"))) for line in lines] == [
        ("button", "south", True),
        ("axis", "lx", -0.5),
        ("button", "south", False),
    ]


@pytest.mark.parametrize("look", ["reprise"], indirect=True)
def test_expect_quits_unless_return_would_launch_that_game(look, monkeypatch):
    exits = []
    monkeypatch.setattr(gamepad, "QCoreApplication", SimpleNamespace(exit=exits.append))
    look.home()
    shown = until(lambda: gamepad.launch_target(look.window), "no tile's game under Return")
    script = gamepad.KeyScript(f"Expect:{shown} Expect:another-game Right", 1, look.window)
    script._step()
    assert exits == [], "the game Return would launch"
    script._step()
    assert exits == [gamepad.EXPECT_FAILED] and script._queue == [], "nothing after it plays"
    look.press(Qt.Key.Key_End)
    until(lambda: gamepad.launch_target(look.window) == "", "the library tile launches nothing")
    look.press(Qt.Key.Key_Left)
    until(lambda: gamepad.launch_target(look.window), "no game left of the library tile")
    look.press(Qt.Key.Key_I)
    until(lambda: gamepad.launch_target(look.window) is None, "the detail page over the row takes Return")


@pytest.mark.parametrize("look", ["switch2", "ps5"], indirect=True)
def test_expect_quits_on_a_look_that_does_not_say_what_return_launches(look, monkeypatch):
    exits = []
    monkeypatch.setattr(gamepad, "QCoreApplication", SimpleNamespace(exit=exits.append))
    look.home()
    gamepad.KeyScript("Expect:", 1, look.window)._step()
    assert exits == [gamepad.EXPECT_FAILED], "its Return may launch the tile it is on"


def test_a_mark_logs_its_name_with_the_time(app, caplog, monkeypatch):
    monkeypatch.setattr(gamepad.time, "time", lambda: 1234.5)
    with caplog.at_level("INFO", logger="universe.gamepad"):
        gamepad.KeyScript("Mark:launch", 1, None)._step()
    assert "mark launch 1234.500" in caplog.messages


def test_return_in_homes_dock_launches_nothing_and_another_window_is_unknown(monkeypatch):
    focused = SimpleNamespace(objectName=lambda: "homeOverlay")
    monkeypatch.setattr(gamepad, "QGuiApplication", SimpleNamespace(focusWindow=lambda: focused))
    assert gamepad.launch_target(object()) == ""
    focused.objectName = lambda: "dialog"
    assert gamepad.launch_target(object()) is None

