import importlib.machinery
import importlib.util
import json
import sys
from pathlib import Path

import pytest

BIN_DIR = Path(__file__).resolve().parents[1] / "bin"
sys.path.insert(0, str(BIN_DIR))
import _eden  # noqa: E402
from _sdl import BIND_AXIS, BIND_BUTTON, BIND_HAT, Pad  # noqa: E402

B, A, H = BIND_BUTTON, BIND_AXIS, BIND_HAT

# SDL3 under Eden's hints, read off a DualSense Edge over Bluetooth.
EDGE = Pad(
    name="DualSense Edge Wireless Controller",
    guid=bytes.fromhex("0500e0274c050000f20d000000006800"),
    bindings={
        **{(B, i): (B, i, 0) for i in range(11)},
        (B, 11): (H, 0, 1),
        (B, 12): (H, 0, 4),
        (B, 13): (H, 0, 8),
        (B, 14): (H, 0, 2),
        (B, 15): (B, 12, 0),
        **{(A, i): (A, i, 0) for i in range(6)},
    },
    gyro=True,
)
# The Linux joystick driver's numbering of an Xbox pad (xpadneo): triggers on axes 2 and 5.
XBOX = Pad(
    name="Xbox Wireless Controller",
    guid=bytes.fromhex("0500509a5e0400008e02000030110000"),
    bindings={
        (B, 0): (B, 0, 0),
        (B, 1): (B, 1, 0),
        (B, 2): (B, 2, 0),
        (B, 3): (B, 3, 0),
        (B, 4): (B, 6, 0),
        (B, 5): (B, 8, 0),
        (B, 6): (B, 7, 0),
        (B, 7): (B, 9, 0),
        (B, 8): (B, 10, 0),
        (B, 9): (B, 4, 0),
        (B, 10): (B, 5, 0),
        (B, 11): (H, 0, 1),
        (B, 12): (H, 0, 4),
        (B, 13): (H, 0, 8),
        (B, 14): (H, 0, 2),
        (A, 0): (A, 0, 0),
        (A, 1): (A, 1, 0),
        (A, 2): (A, 3, 0),
        (A, 3): (A, 4, 0),
        (A, 4): (A, 2, 0),
        (A, 5): (A, 5, 0),
    },
)

# What Eden's own auto-mapping wrote for that Edge (input/dualsense-edge.ini).
EDEN_EDGE = {
    "button_a": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:1",
    "button_b": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:0",
    "button_x": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:3",
    "button_y": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:2",
    "button_lstick": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:7",
    "button_rstick": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:8",
    "button_l": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:9",
    "button_r": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:10",
    "button_zl": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,axis:4,threshold:0.5,invert:+",
    "button_zr": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,axis:5,threshold:0.5,invert:+",
    "button_plus": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:6",
    "button_minus": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:4",
    "button_dleft": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,hat:0,direction:left",
    "button_dup": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,hat:0,direction:up",
    "button_dright": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,hat:0,direction:right",
    "button_ddown": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,hat:0,direction:down",
    "button_slleft": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:9",
    "button_srleft": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:10",
    "button_slright": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:9",
    "button_srright": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:10",
    "button_home": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,button:5",
    "lstick": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,axis_x:0,axis_y:1,offset_x:-0.011750,offset_y:0.019593,invert_x:+,invert_y:+",
    "rstick": "engine:sdl,port:0,guid:050000004c050000f20d000000006800,axis_x:2,axis_y:3,offset_x:0.043153,offset_y:0.011750,invert_x:+,invert_y:+",
    "motionleft": "engine:sdl,motion:0,port:0,guid:050000004c050000f20d000000006800",
    "motionright": "engine:sdl,motion:0,port:0,guid:050000004c050000f20d000000006800",
}


def params(value):
    """Eden's ParamPackage is a map, and a stick's offsets are where it rested when mapped."""
    if value == _eden.EMPTY:
        return {}
    pairs = dict(p.split(":", 1) for p in value.split(","))
    return {k: v for k, v in pairs.items() if not k.startswith("offset_")}


def player(values, n=0):
    prefix = f"player_{n}_"
    return {k[len(prefix) :]: v for k, v in values.items() if k.startswith(prefix)}


def test_positional_is_what_edens_own_mapping_writes_less_home():
    got = player(_eden.values_for([EDGE], "positional", guide=True))
    for key, want in EDEN_EDGE.items():
        assert params(got[key]) == params(want), key
    assert got["button_screenshot"] == _eden.EMPTY
    assert got["connected"] == "true" and got["type"] == "0"


def test_home_is_left_to_universe_unless_guide_is_on():
    assert player(_eden.values_for([EDGE], "positional", guide=False))["button_home"] == _eden.EMPTY


def test_xbox_layout_puts_a_on_the_bottom_button():
    got = player(_eden.values_for([EDGE], "xbox", guide=False))
    assert [params(got[f"button_{k}"])["button"] for k in "abxy"] == ["0", "1", "2", "3"]


def test_an_xbox_pad_on_the_joystick_driver_has_axis_triggers_and_no_motion():
    got = player(_eden.values_for([XBOX], "positional", guide=False))
    assert params(got["button_zl"]) == {
        "engine": "sdl",
        "port": "0",
        "guid": "050000005e0400008e02000030110000",
        "axis": "2",
        "threshold": "0.5",
        "invert": "+",
    }
    assert params(got["button_l"])["button"] == "4" and params(got["button_plus"])["button"] == "7"
    assert (params(got["rstick"])["axis_x"], params(got["rstick"])["axis_y"]) == ("3", "4")
    assert got["motionleft"] == got["motionright"] == _eden.EMPTY


def test_players_follow_the_pads_and_the_rest_are_unplugged():
    twin = Pad(name=EDGE.name, guid=bytes.fromhex("0500ffff4c050000f20d000000006800"), bindings=EDGE.bindings, gyro=True)
    got = _eden.values_for([EDGE, XBOX, twin], "positional", guide=False)
    assert _eden.ports([EDGE, XBOX, twin]) == [0, 0, 1]
    assert params(got["player_2_button_a"])["port"] == "1"
    assert params(got["player_1_button_a"])["guid"] == "050000005e0400008e02000030110000"
    assert [got[f"player_{n}_connected"] for n in range(8)] == ["true"] * 3 + ["false"] * 5
    assert not any(k.startswith(("player_8_", "player_9_")) for k in got)


def test_driver_hints_follow_edens_own_switch_drivers():
    assert _eden.driver_hints({}) == {"SDL_JOYSTICK_HIDAPI_XBOX": "0", "SDL_JOYSTICK_HIDAPI_JOY_CONS": "0", "SDL_JOYSTICK_HIDAPI_SWITCH": "1"}
    hints = _eden.driver_hints({"enable_joycon_driver": "false", "enable_procon_driver": "true"})
    assert hints["SDL_JOYSTICK_HIDAPI_JOY_CONS"] == "1" and hints["SDL_JOYSTICK_HIDAPI_SWITCH"] == "0"


INI = """[DisabledAddOns]
size=0

[Controls]
enable_joycon_driver\\default=true
enable_joycon_driver=true
player_0_button_a\\default=true
player_0_button_a="engine:keyboard,code:67,toggle:0"
player_0_vibration_enabled=true

[UI]
Shortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq=Home+Minus
"""


def test_rewrite_sets_owned_keys_in_place_and_keeps_every_other_line():
    new = _eden.rewrite(INI, {"player_0_button_a": "engine:sdl,port:0,guid:00,button:1", "player_0_button_home": _eden.EMPTY, "player_0_connected": "true"})
    lines = new.splitlines()
    assert 'player_0_button_a="engine:sdl,port:0,guid:00,button:1"' in lines
    assert "player_0_button_a\\default=false" in lines
    assert "player_0_button_home=[empty]" in lines and "player_0_connected=true" in lines
    assert lines.index("player_0_button_home=[empty]") < lines.index("[UI]")
    assert "player_0_vibration_enabled=true" in lines and "Shortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq=Home+Minus" in lines
    assert _eden.section(new)["player_0_button_a"] == "engine:sdl,port:0,guid:00,button:1"
    assert _eden.rewrite(new, {"player_0_button_a": "engine:sdl,port:0,guid:00,button:1"}) == new


def _script(name):
    loader = importlib.machinery.SourceFileLoader(f"controls_{name}", str(BIN_DIR / name))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


@pytest.fixture
def hook(tmp_path, monkeypatch):
    pre = _script("pre")
    config = tmp_path / "config"
    (config / "eden").mkdir(parents=True)
    (config / "eden" / "qt-config.ini").write_text(INI)
    monkeypatch.setenv("XDG_CONFIG_HOME", str(config))
    monkeypatch.setenv("GAME_ID", "mk8")
    monkeypatch.setenv("MODULE_SETTINGS_JSON", json.dumps({"layout": "xbox"}))
    monkeypatch.setattr(pre, "pads_module_runs", lambda _game: False)
    monkeypatch.setattr(pre._sdl, "pads", lambda _hints: [EDGE])

    def run(effective):
        monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps({"effective": effective}))
        return pre.main()

    run.ini = config / "eden" / "qt-config.ini"
    run.pre = pre
    return run


EDEN = {"runner": "eden", "runner_kind": "emulator", "inputplumber": False}


def test_pre_writes_edens_controls_and_backs_the_file_up_once(hook):
    assert hook(EDEN) == 0
    controls = _eden.section(hook.ini.read_text())
    assert params(controls["player_0_button_a"])["button"] == "0"
    assert controls["player_1_connected"] == "false"
    assert hook.ini.with_name("qt-config.ini.before-universe").read_text() == INI


@pytest.mark.parametrize(
    "effective",
    [
        {**EDEN, "runner": "dolphin"},
        {**EDEN, "runner": "proton", "runner_kind": "proton"},
        {**EDEN, "inputplumber": True},
    ],
)
def test_pre_leaves_the_file_alone_where_it_does_not_apply(hook, effective):
    assert hook(effective) == 0
    assert hook.ini.read_text() == INI


def test_pre_stands_down_for_the_pads_module_and_without_pads(hook, monkeypatch):
    monkeypatch.setattr(hook.pre, "pads_module_runs", lambda _game: True)
    assert hook(EDEN) == 0 and hook.ini.read_text() == INI
    monkeypatch.setattr(hook.pre, "pads_module_runs", lambda _game: False)
    monkeypatch.setattr(hook.pre._sdl, "pads", lambda _hints: [])
    assert hook(EDEN) == 0 and hook.ini.read_text() == INI
