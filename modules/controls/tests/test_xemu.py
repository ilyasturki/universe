import tomllib

import _xemu
import pytest
from _controls import Context, ini_section
from controls_fixtures import EDGE, XBOX, twin

TOML = """[general]
show_welcome = false

[input]
gamepad_mappings = [
    { gamepad_id = '050000004c050000e60c000000810000', controller_mapping = { guide = -1 } }
    ]

[input.bindings]
port1_driver = 'usb-xbox-gamepad-s'
port1 = '050000004c050000e60c000000810000'
port2 = '030000005e040000120b000011050000'
port3 = 'keyboard'

[sys.files]
bootrom_path = '/games/xbox/mcpx.bin'
"""


@pytest.fixture
def toml(tmp_path):
    path = tmp_path / "xemu" / "xemu" / "xemu.toml"
    path.parent.mkdir(parents=True)
    path.write_text(TOML)
    return path


def test_ports_follow_the_pads_and_stale_ones_are_emptied(toml):
    text = _xemu.plan(Context([EDGE, XBOX]))[toml]
    got = ini_section(text, "input.bindings")
    assert got["port1"] == f"'{EDGE.guid.hex()}'" and got["port2"] == f"'{XBOX.guid.hex()}'"
    assert got["port1_driver"] == "'usb-xbox-gamepad-s'" and got["port2_driver"] == "'usb-xbox-gamepad'"
    assert got["port3"] == "'keyboard'" and got["port4"] == "''"
    assert "bootrom_path = '/games/xbox/mcpx.bin'" in text and "gamepad_mappings = [" in text


def test_two_identical_pads_share_a_guid_on_two_ports(toml):
    got = ini_section(_xemu.plan(Context([EDGE, twin(EDGE)]))[toml], "input.bindings")
    assert got["port1"] == got["port2"] == f"'{EDGE.guid.hex()}'"


def test_the_file_parses_as_toml_after_the_write(toml):
    doc = tomllib.loads(_xemu.plan(Context([EDGE]))[toml])
    assert doc["input"]["bindings"]["port1"] == EDGE.guid.hex() and doc["input"]["bindings"]["port2"] == ""


def test_a_table_is_made_when_xemu_has_none(toml):
    toml.write_text("[general]\nshow_welcome = false\n")
    assert tomllib.loads(_xemu.plan(Context([XBOX]))[toml])["input"]["bindings"]["port1"] == XBOX.guid.hex()
