import _melonds
from _controls import Context, ini_section, taken_buttons
from _gamepads import BIND_AXIS, BIND_BUTTON, Input, Pad
from controls_fixtures import EDGE, XBOX


def test_face_buttons_follow_the_layout():
    positional = _melonds.values_for(Context([EDGE]))
    assert [positional[k] for k in "ABXY"] == [1, 0, 3, 2]
    xbox = _melonds.values_for(Context([EDGE], "xbox"))
    assert [xbox[k] for k in "ABXY"] == [0, 1, 2, 3]
    assert (xbox["Select"], xbox["Start"]) == (4, 6)


def test_the_dpad_also_reads_the_left_stick():
    got = _melonds.values_for(Context([EDGE]))
    assert (got["Up"], got["Down"], got["Left"], got["Right"]) == (17891585, 16843012, 1114376, 65794)


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


TOML = """[DS]
BIOS9Path = ""

[Instance0]
JoystickID = 1
WindowCount = 1

[Instance0.Keyboard]
A = 88
HK_Pause = 32

[Instance0.Joystick]
Up = 257
HK_Pause = 69271560
HK_FastForward = 86048777
A = 0

[Instance0.DS]
"""


def test_rewrite_sets_the_pad_and_leaves_the_keyboard_and_the_rest():
    new = _melonds.rewrite(TOML, Context([XBOX]))
    assert ini_section(new, "Instance0") == {"JoystickID": "1", "WindowCount": "1"}
    joystick = ini_section(new, "Instance0.Joystick")
    assert joystick["A"] == "1" and joystick["Up"] == str(0x100 | 1 | 0x10000 | 1 << 20 | 1 << 24)
    assert joystick["HK_Pause"] == str(0x0421FFFF) and joystick["HK_FastForward"] == "86048777"
    assert ini_section(new, "Instance0.Keyboard") == {"A": "88", "HK_Pause": "32"}
    assert "JoystickID = 1" in new and "[Instance0.DS]" in new


def test_joystick_id_is_the_pads_place_among_all_joysticks():
    assert ini_section(_melonds.rewrite(TOML, Context([EDGE])), "Instance0")["JoystickID"] == "0"


def test_l_and_r_read_the_bumper_and_the_trigger():
    got = _melonds.values_for(Context([EDGE]))
    trigger = 0x10000 | 2 << 20
    assert (got["L"], got["R"]) == (9 | trigger | 4 << 24, 10 | trigger | 5 << 24)
    assert _melonds.values_for(Context([EDGE], shoulders="swapped"))["L"] == got["L"]


def test_digital_triggers_leave_l_to_the_bumper_unless_swapped():
    digital = Pad(**{**vars(EDGE), "bindings": {**EDGE.bindings, (BIND_AXIS, 4): Input(BIND_BUTTON, 12)}})
    assert _melonds.values_for(Context([digital]))["L"] == 9
    assert _melonds.values_for(Context([digital], shoulders="swapped"))["L"] == 12
