import _rpcs3
import pytest
from _controls import Context
from controls_fixtures import EDGE, XBOX, twin

USER_DEFAULT = """Player 1 Input:
  Handler: DualSense
  Device: "DualSense Pad #1"
  Config:
    Cross: Cross
    PS Button: PS Button
    Left Stick Deadzone: 40
  Buddy Device: ""
Player 2 Input:
  Handler: "Null"
  Device: "Null"
  Buddy Device: ""
"""

PER_TITLE = """Player 1 Input:
  Handler: SDL
  Device: "PS4 Controller 1"
  Config:
    Cross: "East"
    Circle: "South"
  Buddy Device: ""
Player 2 Input:
  Handler: Evdev
  Device: "Some Evdev Pad"
  Buddy Device: ""
Player 3 Input:
  Handler: SDL
  Device: "Old Pad 1"
  Buddy Device: ""
"""


@pytest.fixture
def rpcs3(tmp_path):
    configs = tmp_path / "rpcs3" / "input_configs"
    (configs / "global").mkdir(parents=True)
    (configs / "global" / "Default.yml").write_text(USER_DEFAULT)
    return configs


def blocks(text):
    out, current = {}, None
    for line in text.splitlines():
        if line.startswith("Player "):
            current = int(line.split()[1])
            out[current] = {}
        elif current and ":" in line:
            k, v = line.split(":", 1)
            out[current][k] = v.strip()
    return out


def test_the_active_global_config_is_written_fresh_for_the_sdl_handler_counting_pads_by_name(rpcs3):
    files = _rpcs3.plan(Context([EDGE, XBOX, twin(EDGE)]))
    got = blocks(files[rpcs3 / "global" / "Default.yml"])
    assert got[1]["  Handler"] == "SDL" and got[1]["  Device"] == '"DualSense Edge Wireless Controller 1"'
    assert got[1]["    Cross"] == '"South"' and got[1]["    Triangle"] == '"North"' and got[1]["    Left Stick Up"] == '"LS Y+"'
    assert (got[1]["    L1"], got[1]["    L2"], got[1]["    PS Button"]) == ('"LB"', '"LT"', '""')
    assert got[1]["    Product ID"] == "616" and "    Left Stick Deadzone" not in got[1]
    assert (got[2]["  Device"], got[3]["  Device"]) == ('"Xbox Wireless Controller 1"', '"DualSense Edge Wireless Controller 2"')
    assert [got[n]["  Handler"] for n in range(4, 8)] == ['"Null"'] * 4


def test_guide_goes_to_the_ps_button_when_passed_through(rpcs3):
    text = _rpcs3.plan(Context([EDGE], guide=True))[rpcs3 / "global" / "Default.yml"]
    assert blocks(text)[1]["    PS Button"] == '"Guide"'


def test_the_active_configuration_names_the_file(rpcs3):
    (rpcs3 / "active_input_configurations.yml").write_text("Active Configurations:\n  global: Couch\n  BLES01807: Default\n")
    files = _rpcs3.plan(Context([EDGE]))
    assert rpcs3 / "global" / "Couch.yml" in files
    assert rpcs3 / "global" / "Default.yml" not in files


def test_per_title_configs_keep_their_bindings_and_follow_the_held_pads(rpcs3):
    (rpcs3 / "BLES01807").mkdir()
    (rpcs3 / "BLES01807" / "Default.yml").write_text(PER_TITLE)
    new = _rpcs3.plan(Context([EDGE, XBOX]))[rpcs3 / "BLES01807" / "Default.yml"]
    got = blocks(new)
    assert got[1]["  Device"] == '"DualSense Edge Wireless Controller 1"' and got[1]["    Cross"] == '"East"'
    assert got[2]["  Device"] == '"Some Evdev Pad"'
    assert got[3]["  Device"] == '"Old Pad 1"'
    assert new.replace('"DualSense Edge Wireless Controller 1"', '"PS4 Controller 1"') == PER_TITLE


def test_hints_put_a_ds3_on_hidapi_and_load_rpcs3s_database(rpcs3):
    assert _rpcs3.hints() == {"SDL_JOYSTICK_HIDAPI_PS3": "1"}
    (rpcs3 / "gamecontrollerdb.txt").write_text("")
    assert _rpcs3.databases() == [rpcs3 / "gamecontrollerdb.txt"]
