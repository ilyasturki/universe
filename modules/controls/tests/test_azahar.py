import _azahar
from _controls import Context, ini_section
from _gamepads import BIND_AXIS, Input
from controls_fixtures import EDGE, XBOX, twin

G = "engine:sdl,guid:0500e0274c050000f20d000000006800,port:0"


def params(value):
    return dict(p.split(":", 1) for p in value.split(",")) if value != _azahar.EMPTY else {}


def test_positional_matches_azahars_own_automap_less_home():
    got = _azahar.values_for(Context([EDGE]))
    assert got["button_a"] == f"button:1,{G}"
    assert got["button_b"] == f"button:0,{G}"
    assert (got["button_x"], got["button_y"]) == (f"button:3,{G}", f"button:2,{G}")
    assert got["button_up"] == "direction:up,engine:sdl,guid:0500e0274c050000f20d000000006800,hat:0,port:0"
    assert (got["button_l"], got["button_r"], got["button_start"], got["button_select"]) == tuple(f"button:{n},{G}" for n in (9, 10, 6, 4))
    assert got["button_zl"] == "axis:4,direction:+,engine:sdl,guid:0500e0274c050000f20d000000006800,port:0,threshold:0.000000"
    assert got["circle_pad"] == "axis_x:0,axis_y:1,deadzone:0.100000,engine:sdl,guid:0500e0274c050000f20d000000006800,port:0"
    assert got["c_stick"] == "axis_x:2,axis_y:3,deadzone:0.100000,engine:sdl,guid:0500e0274c050000f20d000000006800,port:0"
    assert got["motion_device"] == G
    assert got["button_home"] == _azahar.EMPTY


def test_guide_passed_through_binds_home_and_a_pad_without_gyro_gets_emulated_motion():
    got = _azahar.values_for(Context([XBOX], guide=True))
    assert params(got["button_home"])["button"] == "8"
    assert params(got["button_zr"])["axis"] == "5"
    assert got["motion_device"] == "engine:motion_emu,sensitivity:0.01,tilt_clamp:90.0,update_period:100"


def test_a_twin_made_player_1_keeps_the_port_sdl_gave_it():
    assert params(_azahar.values_for(Context([twin(EDGE), XBOX, EDGE]))["button_a"])["port"] == "1"


def test_a_half_axis_trigger_presses_past_the_middle():
    half = Input(BIND_AXIS, 4, lo=0, hi=32767)
    assert params(_azahar._param({"engine": "sdl"}, half))["threshold"] == "0.500000"
    inverted = Input(BIND_AXIS, 4, lo=32767, hi=-32768)
    assert params(_azahar._param({"engine": "sdl"}, inverted))["direction"] == "-"


def test_package_sorts_keys_and_escapes():
    assert _azahar.package({"z": "a:b,c$", "a": 1}) == "a:1,z:a$0b$1c$2"
    assert _azahar.package({}) == _azahar.EMPTY


INI = """[Controls]
profile\\default=true
profile=1
profiles\\1\\button_a\\default=true
profiles\\1\\button_a="code:65,engine:keyboard"
profiles\\2\\button_a="code:65,engine:keyboard"
profiles\\2\\button_debug="code:79,engine:keyboard"
profiles\\2\\name=Pad
profiles\\size=2
"""


def test_rewrite_fills_the_active_profile_and_grows_the_list_to_hold_it():
    new = _azahar.rewrite(INI, _azahar.values_for(Context([EDGE])))
    controls = ini_section(new, "Controls")
    assert controls["profiles\\2\\button_a"] == f"button:1,{G}" and controls["profiles\\2\\button_a\\default"] == "false"
    assert controls["profiles\\2\\button_home"] == "[empty]"
    assert controls["profiles\\1\\button_a"] == "code:65,engine:keyboard"
    assert controls["profiles\\2\\button_debug"] == "code:79,engine:keyboard" and controls["profiles\\size"] == "2"
    assert 'profiles\\2\\button_a="button:1,' in new and "profiles\\2\\button_home=[empty]" in new
    grown = _azahar.rewrite("[Controls]\nprofile=0\n", {"button_a": _azahar.EMPTY})
    assert ini_section(grown, "Controls")["profiles\\size"] == "1"
