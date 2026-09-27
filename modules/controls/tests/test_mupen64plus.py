import _mupen64plus
import pytest
from _controls import Context, Skip, ini_section
from controls_fixtures import EDGE, SWITCH_PRO, XBOX


@pytest.fixture
def cfg(tmp_path):
    return tmp_path / "mupen64plus" / "mupen64plus.cfg"


def test_an_xbox_pad_matches_mupens_own_series_x_block(cfg):
    """InputAutoCfg.ini's Xbox Series X entry, with the device index of this pad."""
    text = _mupen64plus.plan(Context([XBOX]))[cfg]
    assert ini_section(text, "Input-SDL-Control1") == {
        "version": "2.000000",
        "mode": "0",
        "device": "1",
        "name": "Xbox Wireless Controller",
        "plugged": "True",
        "plugin": "2",
        "mouse": "False",
        "MouseSensitivity": "2.00,2.00",
        "AnalogDeadzone": "4096,4096",
        "AnalogPeak": "32768,32768",
        "DPad R": "hat(0 Right)",
        "DPad L": "hat(0 Left)",
        "DPad D": "hat(0 Down)",
        "DPad U": "hat(0 Up)",
        "Start": "button(7)",
        "Z Trig": "axis(2+)",
        "B Button": "button(2)",
        "A Button": "button(0)",
        "C Button R": "axis(3+)",
        "C Button L": "axis(3-)",
        "C Button D": "axis(4+)",
        "C Button U": "axis(4-)",
        "R Trig": "button(5) axis(5+)",
        "L Trig": "button(4)",
        "Mempak switch": "",
        "Rumblepak switch": "",
        "X Axis": "axis(0-,0+)",
        "Y Axis": "axis(1-,1+)",
    }
    assert 'Z Trig = "axis(2+)"' in text.splitlines()


def test_unused_slots_are_unplugged_so_auto_mode_grabs_nothing(cfg):
    text = _mupen64plus.plan(Context([EDGE, SWITCH_PRO]))[cfg]
    assert ini_section(text, "Input-SDL-Control2")["device"] == "2"
    for n in (3, 4):
        assert ini_section(text, f"Input-SDL-Control{n}") == {"version": "2.000000", "mode": "0", "device": "-1", "name": "", "plugged": "False"}


def test_the_layout_setting_does_not_move_n64_buttons(cfg):
    a = ini_section(_mupen64plus.plan(Context([EDGE], "xbox"))[cfg], "Input-SDL-Control1")
    b = ini_section(_mupen64plus.plan(Context([EDGE], "positional"))[cfg], "Input-SDL-Control1")
    assert a == b and (a["A Button"], a["B Button"]) == ("button(0)", "button(2)")


def test_old_sections_are_replaced_and_the_rest_kept(cfg):
    cfg.parent.mkdir()
    cfg.write_text(
        '# Mupen64Plus Configuration File\n\n[Core]\nVersion = 1.010000\n\n[Input-SDL-Control1]\n# help\nmode = 2\nname = "Old"\n\n'
        "[Input-SDL-Control2]\nmode = 2\n\n[Video-General]\nFullscreen = False\n"
    )
    text = _mupen64plus.plan(Context([EDGE]))[cfg]
    assert text.startswith("# Mupen64Plus Configuration File\n\n[Core]\nVersion = 1.010000\n")
    assert ini_section(text, "Input-SDL-Control1")["mode"] == "0" and "# help" not in text
    assert ini_section(text, "Input-SDL-Control2")["device"] == "-1"
    assert ini_section(text, "Video-General") == {"Fullscreen": "False"}
    assert text.count("[Input-SDL-Control1]") == 1


def test_m64p_is_left_alone(cfg):
    with pytest.raises(Skip):
        _mupen64plus.plan(Context([EDGE], runner_path="/usr/bin/m64p"))


def test_swapped_shoulders_put_z_on_the_left_bumper(cfg):
    got = ini_section(_mupen64plus.plan(Context([XBOX], shoulders="swapped"))[cfg], "Input-SDL-Control1")
    assert (got["Z Trig"], got["L Trig"], got["R Trig"]) == ("button(4)", "axis(2+)", "axis(5+) button(5)")
