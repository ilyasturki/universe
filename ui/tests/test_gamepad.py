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
    watcher.event.connect(lines.append)
    script = gamepad.KeyScript("Press:south Axis:lx=-0.5 Unpress:south", 1, None, watcher=watcher)
    for _ in range(3):
        script._step()
    assert [(l["event"], l.get("slot") or l.get("axis"), l.get("pressed", l.get("value"))) for l in lines] == [
        ("button", "south", True), ("axis", "lx", -0.5), ("button", "south", False)]
