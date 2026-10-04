import tomllib

import _xemu
import pytest
from _controls import Context
from controls_fixtures import EDGE, XBOX, twin

TOML = """[input.bindings]
port1_driver = 'usb-xbox-gamepad-s'
port1 = '050000004c050000e60c000000810000'
port2 = '030000005e040000120b000011050000'
port3 = 'keyboard'
"""


@pytest.fixture
def toml(tmp_path):
    path = tmp_path / "xemu" / "xemu" / "xemu.toml"
    path.parent.mkdir(parents=True)
    path.write_text(TOML)
    return path


def test_ports_follow_the_pads_even_two_of_one_model_and_stale_ones_are_emptied(toml):
    got = tomllib.loads(_xemu.plan(Context([EDGE, twin(EDGE)]))[toml])["input"]["bindings"]
    assert got["port1"] == got["port2"] == EDGE.guid.hex()
    assert (got["port1_driver"], got["port2_driver"]) == ("usb-xbox-gamepad-s", "usb-xbox-gamepad")
    assert (got["port3"], got["port4"]) == ("keyboard", "")


def test_a_table_is_made_when_xemu_has_none(toml):
    toml.write_text("[general]\nshow_welcome = false\n")
    assert tomllib.loads(_xemu.plan(Context([XBOX]))[toml])["input"]["bindings"]["port1"] == XBOX.guid.hex()
