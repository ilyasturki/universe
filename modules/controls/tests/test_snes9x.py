import _snes9x
import pytest
from _controls import Context, Skip, ini_section
from controls_fixtures import EDGE, SWITCH_PRO, XBOX

GTK = "/nix/store/x-snes9x-gtk/bin/snes9x-gtk"


@pytest.fixture
def conf(tmp_path):
    return tmp_path / "snes9x" / "snes9x.conf"


def test_the_x11_build_is_left_alone(conf):
    with pytest.raises(Skip):
        _snes9x.plan(Context([EDGE], runner_path="/usr/bin/snes9x"))


def test_a_fresh_file_holds_the_ports_and_both_sets(conf):
    text = _snes9x.plan(Context([EDGE], runner_path=GTK))[conf]
    assert ini_section(text, "Input") == {"ControllerPort0": "joypad", "ControllerPort1": "joypad"}
    pad = ini_section(text, "Joypad 0")
    assert (pad["A"], pad["B"], pad["X"], pad["Y"]) == ("Joystick 1 Button 1", "Joystick 1 Button 0", "Joystick 1 Button 3", "Joystick 1 Button 2")
    assert (pad["L"], pad["R"], pad["Select"], pad["Start"]) == ("Joystick 1 Button 9", "Joystick 1 Button 10", "Joystick 1 Button 4", "Joystick 1 Button 6")
    # A hat after the Edge's 6 axes: up and down on pseudo-axis 6, left and right on 7.
    assert (pad["Up"], pad["Down"], pad["Left"], pad["Right"]) == (
        "Joystick 1 Axis 6 + 50%",
        "Joystick 1 Axis 6 - 50%",
        "Joystick 1 Axis 7 - 50%",
        "Joystick 1 Axis 7 + 50%",
    )
    assert pad["Turbo A"] == pad["Sticky R"] == "Unset"
    stick = ini_section(text, "Joypad 5")
    assert (stick["Up"], stick["Down"], stick["Left"], stick["Right"]) == (
        "Joystick 1 Axis 1 - 50%",
        "Joystick 1 Axis 1 + 50%",
        "Joystick 1 Axis 0 - 50%",
        "Joystick 1 Axis 0 + 50%",
    )
    assert stick["A"] == "Unset"


def test_xbox_layout_and_the_joystick_index(conf):
    text = _snes9x.plan(Context([XBOX], "xbox", runner_path=GTK))[conf]
    pad = ini_section(text, "Joypad 0")
    assert (pad["A"], pad["B"], pad["X"], pad["Y"], pad["Start"]) == (
        "Joystick 2 Button 0",
        "Joystick 2 Button 1",
        "Joystick 2 Button 2",
        "Joystick 2 Button 3",
        "Joystick 2 Button 7",
    )


def test_a_multitap_only_for_three_pads(conf):
    text = _snes9x.plan(Context([EDGE, XBOX, SWITCH_PRO], runner_path=GTK))[conf]
    assert ini_section(text, "Input")["ControllerPort1"] == "multitap"
    assert ini_section(text, "Joypad 2")["A"] == "Joystick 3 Button 1"


def test_stale_pads_and_shortcuts_on_universes_buttons_are_unset(conf):
    conf.parent.mkdir()
    conf.write_text(
        "[Joypad 1]\nA = Joystick 2 Button 1\nB = Keyboard x\n\n[Joypad 6]\nUp = Joystick 2 Axis 1 - 50%\n\n"
        "[Shortcuts]\nGTK_quit = Joystick 1 Button 5\nGTK_fullscreen = Joystick 1 Button 3\nQuickSave000 = Keyboard F1\n\n[Display]\nFullscreen = true\n"
    )
    text = _snes9x.plan(Context([EDGE], runner_path=GTK))[conf]
    assert ini_section(text, "Joypad 1") == {"A": "Unset", "B": "Keyboard x"}
    assert ini_section(text, "Joypad 6") == {"Up": "Unset"}
    assert ini_section(text, "Shortcuts") == {"GTK_quit": "Unset", "GTK_fullscreen": "Joystick 1 Button 3", "QuickSave000": "Keyboard F1"}
    assert ini_section(text, "Display") == {"Fullscreen": "true"}


def test_the_second_set_puts_l_and_r_on_the_triggers(conf):
    text = _snes9x.plan(Context([EDGE], runner_path=GTK))[conf]
    assert (ini_section(text, "Joypad 0")["L"], ini_section(text, "Joypad 5")["R"]) == ("Joystick 1 Button 9", "Joystick 1 Axis 5 + 0%")
    swapped = _snes9x.plan(Context([EDGE], runner_path=GTK, shoulders="swapped"))[conf]
    assert (ini_section(swapped, "Joypad 0")["L"], ini_section(swapped, "Joypad 5")["L"]) == ("Joystick 1 Axis 4 + 0%", "Joystick 1 Button 9")
