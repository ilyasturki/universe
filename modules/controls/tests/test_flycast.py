import _flycast
import pytest
from _controls import Context, ini_section
from controls_fixtures import EDGE, SWITCH_PRO, XBOX

CFG = """[config]
rend.Resolution = 960

[input]
device1 = 0
device1.1 = 1
device3 = 7
maple_sdl_joystick_12 = 1
maple_sdl_joystick_3 = 0
maple_sdl_keyboard = 0
maple_sdl_mouse = 0
"""


@pytest.fixture
def cfg(tmp_path):
    path = tmp_path / "flycast" / "emu.cfg"
    path.parent.mkdir()
    path.write_text(CFG)
    return path


def test_instance_ports_go_and_held_pads_get_a_controller(cfg):
    text = _flycast.plan(Context([EDGE, XBOX, SWITCH_PRO]))[cfg]
    assert ini_section(text, "input") == {
        "device1": "0",
        "device1.1": "1",
        "device3": "7",
        "maple_sdl_keyboard": "0",
        "maple_sdl_mouse": "0",
        "device2": "0",
    }
    assert ini_section(text, "config") == {"rend.Resolution": "960"}


def test_nothing_to_write_when_it_already_matches(cfg):
    cfg.write_text("[input]\ndevice1 = 0\ndevice2 = 0\n")
    assert _flycast.plan(Context([EDGE, XBOX])) == {}
