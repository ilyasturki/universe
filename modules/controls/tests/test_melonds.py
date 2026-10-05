import _melonds
from _controls import Context, ini_section, taken_buttons
from _gamepads import BIND_AXIS, Input
from controls_fixtures import EDGE, SWITCH_PRO_DIGITAL, XBOX

TRIGGER = 0x10000 | 2 << 20


def test_the_dpad_also_reads_the_left_stick_and_l_r_read_the_bumper_and_the_trigger():
    assert _melonds.values_for(Context([EDGE])) == {
        "A": 1,
        "B": 0,
        "X": 3,
        "Y": 2,
        "Select": 4,
        "Start": 6,
        "L": 9 | TRIGGER | 4 << 24,
        "R": 10 | TRIGGER | 5 << 24,
        "Up": 17891585,
        "Down": 16843012,
        "Left": 1114376,
        "Right": 65794,
    }


def test_a_trigger_joins_the_bumper_only_as_an_axis():
    assert _melonds.values_for(Context([EDGE]))["L"] == 9 | TRIGGER | 4 << 24
    assert _melonds.values_for(Context([SWITCH_PRO_DIGITAL]))["L"] == 9


def test_axis_bound_buttons_and_triggers():
    assert _melonds.encode(Input(BIND_AXIS, 5, lo=-32768, hi=32767)) == 0xFFFF | 0x10000 | 2 << 20 | 5 << 24
    assert _melonds.encode(Input(BIND_AXIS, 2, lo=0, hi=32767)) == 0xFFFF | 0x10000 | 2 << 24
    assert _melonds.encode(Input(BIND_AXIS, 2, lo=0, hi=-32768)) == 0xFFFF | 0x10000 | 1 << 20 | 2 << 24
    assert _melonds.encode(None) == -1


def test_hotkeys_on_universes_buttons_lose_that_part():
    taken = taken_buttons(XBOX, guide=False)
    assert taken == {8}
    assert _melonds.cleared(69271560, taken) == 0x0421FFFF
    assert _melonds.cleared(8, taken) == -1
    assert _melonds.cleared(86048777, taken) == 86048777
    assert _melonds.cleared(264, taken) == 264
    assert taken_buttons(XBOX, guide=True) == set()
    assert taken_buttons(EDGE, guide=False) == {5, 12, 16, 15, 14, 13}


TOML = """[Instance0]
JoystickID = 0

[Instance0.Keyboard]
A = 88
HK_Pause = 32

[Instance0.Joystick]
Up = 257
HK_Pause = 69271560
HK_FastForward = 86048777
A = 0
"""


def test_rewrite_points_at_the_pads_place_among_all_joysticks_and_leaves_the_keyboard():
    new = _melonds.rewrite(TOML, Context([XBOX]))
    assert ini_section(new, "Instance0")["JoystickID"] == "1"
    joystick = ini_section(new, "Instance0.Joystick")
    assert joystick["A"] == "1" and joystick["Up"] == str(0x100 | 1 | 0x10000 | 1 << 20 | 1 << 24)
    assert joystick["HK_Pause"] == str(0x0421FFFF) and joystick["HK_FastForward"] == "86048777"
    assert ini_section(new, "Instance0.Keyboard") == {"A": "88", "HK_Pause": "32"}
