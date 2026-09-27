import _eden
from _controls import Context, ini_section
from controls_fixtures import EDGE, XBOX, twin

# What Eden's own auto-mapping wrote for that Edge (input/dualsense-edge.ini).
G = "engine:sdl,port:0,guid:050000004c050000f20d000000006800"
EDEN_EDGE = {
    "button_a": f"{G},button:1",
    "button_b": f"{G},button:0",
    "button_x": f"{G},button:3",
    "button_y": f"{G},button:2",
    "button_lstick": f"{G},button:7",
    "button_rstick": f"{G},button:8",
    "button_l": f"{G},button:9",
    "button_r": f"{G},button:10",
    "button_zl": f"{G},axis:4,threshold:0.5,invert:+",
    "button_zr": f"{G},axis:5,threshold:0.5,invert:+",
    "button_plus": f"{G},button:6",
    "button_minus": f"{G},button:4",
    "button_dleft": f"{G},hat:0,direction:left",
    "button_dup": f"{G},hat:0,direction:up",
    "button_dright": f"{G},hat:0,direction:right",
    "button_ddown": f"{G},hat:0,direction:down",
    "button_slleft": f"{G},button:9",
    "button_srleft": f"{G},button:10",
    "button_slright": f"{G},button:9",
    "button_srright": f"{G},button:10",
    "button_home": f"{G},button:5",
    "lstick": f"{G},axis_x:0,axis_y:1,offset_x:-0.011750,offset_y:0.019593,invert_x:+,invert_y:+",
    "rstick": f"{G},axis_x:2,axis_y:3,offset_x:0.043153,offset_y:0.011750,invert_x:+,invert_y:+",
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
    got = player(_eden.values_for(Context([EDGE], "positional", True)))
    for key, want in EDEN_EDGE.items():
        assert params(got[key]) == params(want), key
    assert got["button_screenshot"] == _eden.EMPTY
    assert got["connected"] == "true" and got["type"] == "0"


def test_home_is_left_to_universe_unless_guide_is_on():
    assert player(_eden.values_for(Context([EDGE], "positional", False)))["button_home"] == _eden.EMPTY


def test_xbox_layout_puts_a_on_the_bottom_button():
    got = player(_eden.values_for(Context([EDGE], "xbox", False)))
    assert [params(got[f"button_{k}"])["button"] for k in "abxy"] == ["0", "1", "2", "3"]


def test_an_xbox_pad_on_the_joystick_driver_has_axis_triggers_and_no_motion():
    got = player(_eden.values_for(Context([XBOX], "positional", False)))
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
    pads = [EDGE, XBOX, twin(EDGE)]
    got = _eden.values_for(Context(pads, "positional", False))
    assert _eden.ports(pads) == [0, 0, 1]
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
    assert ini_section(new, "Controls")["player_0_button_a"] == "engine:sdl,port:0,guid:00,button:1"
    assert _eden.rewrite(new, {"player_0_button_a": "engine:sdl,port:0,guid:00,button:1"}) == new


def test_swapped_shoulders_exchange_l_r_with_zl_zr():
    got = player(_eden.values_for(Context([EDGE], shoulders="swapped")))
    assert params(got["button_l"])["axis"] == "4" and params(got["button_slleft"])["axis"] == "4"
    assert params(got["button_zr"])["button"] == "10"
