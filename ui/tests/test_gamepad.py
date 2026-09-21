from PySide6.QtCore import Qt

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
    from PySide6.QtCore import Qt

    from universe_ui.gamepad import AXIS_LEFTX, BTN_A, Mapper

    m = Mapper(Clock())
    m.button(BTN_A, True)
    m.axis(AXIS_LEFTX, 32767)
    assert m.release_all() == [(Qt.Key.Key_Return, False, False), (Qt.Key.Key_Right, False, False)]
    assert m.held == {} and m.release_all() == []
    assert m.axis(AXIS_LEFTX, 32767) == [(Qt.Key.Key_Right, True, False)], "the axis is forgotten: the next push presses again"


def test_post_key_needs_a_focus_window(app):
    assert gamepad.post_key(Qt.Key.Key_Return, True) is False


def test_a_muted_pad_posts_releases_only(app, monkeypatch):
    from universe_ui.api import Pad

    posted = []
    monkeypatch.setattr(gamepad, "post_key", lambda key, pressed, autorepeat=False, window=None: posted.append((key, pressed)))
    pad = Pad()
    thread = gamepad.GamepadThread(pad=pad)
    thread._post(int(Qt.Key.Key_Return), True, False)
    pad.muted = True
    thread._post(int(Qt.Key.Key_Escape), True, False)
    thread._post(int(Qt.Key.Key_Return), False, False)
    pad.muted = False
    thread._post(int(Qt.Key.Key_Escape), True, False)
    assert posted == [(Qt.Key.Key_Return, True), (Qt.Key.Key_Return, False), (Qt.Key.Key_Escape, True)]


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
