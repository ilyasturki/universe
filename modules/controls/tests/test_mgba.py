import _mgba
import pytest
from _controls import Context, ini_section, taken_buttons
from _gamepads import BIND_AXIS, BIND_BUTTON, Input, Pad
from controls_fixtures import EDGE, XBOX, twin

EDGE_POSITIONAL = {
    "keyA": "1",
    "keyB": "0",
    "keySelect": "4",
    "keyStart": "6",
    "keyL": "9",
    "keyR": "10",
    "keyUp": "-1",
    "keyDown": "-1",
    "keyLeft": "-1",
    "keyRight": "-1",
    "hat0Up": "6",
    "hat0Right": "4",
    "hat0Down": "7",
    "hat0Left": "5",
    "axisLeftAxis": "-0",
    "axisLeftValue": "-16384",
    "axisRightAxis": "+0",
    "axisRightValue": "16384",
    "axisUpAxis": "-1",
    "axisUpValue": "-16384",
    "axisDownAxis": "+1",
    "axisDownValue": "16384",
    "axisLAxis": "+4",
    "axisLValue": "-1",
    "axisRAxis": "+5",
    "axisRValue": "-1",
}

CONFIG = """[gba.input.QT_K]
keyA=88

[gba.input.SDLB]
keyA=0
keyB=1
hat1Up=6
axisLeftAxis=-0
axisLeftValue=-12288
device0=05008fe54c050000cc09000000006800
device3=05008fe54c050000cc09000000006800
tiltAxisX=2

[gba.input-profile.DualSense Edge Wireless Controller]
keyA=0
hat0Up=6
gyroSensitivity=2,2e+09

[ports.qt]
fullscreen=1
"""

QT = """[shortcutProfileButton.DualSense%20Edge%20Wireless%20Controller]
holdFastForward=12
quickSave.1=5
saveState=-1

[shortcutProfileAxis.DualSense%20Edge%20Wireless%20Controller]
holdFastForward=+5
"""


@pytest.fixture
def mgba(tmp_path):
    (tmp_path / "mgba").mkdir()
    (tmp_path / "mgba" / "config.ini").write_text(CONFIG)
    (tmp_path / "mgba" / "qt.ini").write_text(QT)
    return tmp_path / "mgba"


def test_bindings_follow_the_research_example():
    assert {k: str(v) for k, v in _mgba.bindings(EDGE, Context([EDGE])).items()} == EDGE_POSITIONAL


def test_xbox_layout_swaps_a_and_b_only():
    got = _mgba.bindings(EDGE, Context([EDGE], "xbox"))
    assert (got["keyA"], got["keyB"], got["keyL"]) == (0, 1, 9)


def test_player_one_and_every_pads_profiles_are_rewritten(mgba):
    files = _mgba.plan(Context([EDGE, XBOX, twin(EDGE)]))
    text = files[mgba / "config.ini"]
    sdlb = ini_section(text, _mgba.SDLB)
    assert {k: v for k, v in sdlb.items() if k.startswith(("key", "axis", "hat"))} == EDGE_POSITIONAL
    assert (sdlb["device0"], sdlb["device1"], sdlb["device2"]) == (EDGE.guid.hex(), XBOX.guid.hex(), EDGE.guid.hex())
    assert "device3" not in sdlb and sdlb["tiltAxisX"] == "2"
    name = ini_section(text, "gba.input-profile.DualSense Edge Wireless Controller")
    assert name["keyA"] == "1" and "hat1Up" not in name and name["gyroSensitivity"] == "2,2e+09"
    assert ini_section(text, f"gba.input-profile.{EDGE.guid.hex()}")["keyA"] == "1"
    xbox = ini_section(text, "gba.input-profile.Xbox Wireless Controller")
    assert (xbox["keyL"], xbox["keyStart"], xbox["axisRightAxis"]) == ("4", "7", "+0")
    assert ini_section(text, "gba.input.QT_K") == {"keyA": "88"} and ini_section(text, "ports.qt") == {"fullscreen": "1"}


def test_shortcuts_on_universes_buttons_are_freed(mgba):
    qt = _mgba.plan(Context([EDGE]))[mgba / "qt.ini"]
    freed = ini_section(qt, "shortcutProfileButton.DualSense%20Edge%20Wireless%20Controller")
    assert freed == {"holdFastForward": "-1", "quickSave.1": "-1", "saveState": "-1"}
    assert ini_section(qt, "shortcutProfileAxis.DualSense%20Edge%20Wireless%20Controller") == {"holdFastForward": "+5"}


def test_the_raw_buttons_universe_keeps():
    assert taken_buttons(EDGE) == {5, 12, 13, 14, 15, 16}
    assert taken_buttons(XBOX) == {8}


def test_qt_escapes_a_group_name_as_qsettings_does():
    assert _mgba.qt_escape("PS4 Controller") == "PS4%20Controller"
    assert _mgba.qt_escape("8BitDo (Pro) é/ž") == "8BitDo%20%28Pro%29%20%E9\\%U017E"


def test_digital_triggers_leave_l_to_the_bumper_unless_swapped():
    digital = Pad(**{**vars(EDGE), "bindings": {**EDGE.bindings, (BIND_AXIS, 4): Input(BIND_BUTTON, 12)}})
    standard = _mgba.bindings(digital, Context([digital]))
    assert standard["keyL"] == 9 and "axisLAxis" not in standard and standard["axisRAxis"] == "+5"
    assert _mgba.bindings(digital, Context([digital], shoulders="swapped"))["keyL"] == 12
