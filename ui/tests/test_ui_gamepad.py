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
    assert m.button(gamepad.BTN_B, True) == [(Qt.Key.Key_Escape, True, False)]
    assert m.button(gamepad.BTN_X, True) == [(Qt.Key.Key_I, True, False)]
    assert m.button(gamepad.BTN_Y, True) == [(Qt.Key.Key_F, True, False)]
    assert m.button(gamepad.BTN_LEFTSHOULDER, True) == [(Qt.Key.Key_Q, True, False)]
    assert m.button(gamepad.BTN_RIGHTSHOULDER, True) == [(Qt.Key.Key_E, True, False)]
    assert m.button(gamepad.BTN_START, True) == [(Qt.Key.Key_F1, True, False)]
    assert m.button(gamepad.BTN_DPAD_LEFT, True) == [(Qt.Key.Key_Left, True, False)]
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


def test_key_script_names_cover_the_pad():
    for name in ("A", "B", "X", "Y", "LB", "RB", "LT", "RT", "Start", "Up", "Down", "Left", "Right"):
        assert name in gamepad.KEY_NAMES
