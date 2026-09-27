import _azahar
from _controls import Context, ini_section
from _gamepads import BIND_AXIS, Input
from controls_fixtures import EDGE, XBOX

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


def test_xbox_layout_guide_and_a_pad_without_gyro():
    got = _azahar.values_for(Context([XBOX], "xbox", guide=True))
    assert [params(got[f"button_{k}"])["button"] for k in "abxy"] == ["0", "1", "2", "3"]
    assert params(got["button_home"])["button"] == "8"
    assert params(got["button_zr"])["axis"] == "5"
    assert got["motion_device"] == "engine:motion_emu,sensitivity:0.01,tilt_clamp:90.0,update_period:100"


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

[UI]
fullscreen=true
"""


def test_rewrite_fills_the_active_profile_and_keeps_the_rest():
    new = _azahar.rewrite(INI, _azahar.values_for(Context([EDGE])))
    controls = ini_section(new, "Controls")
    assert controls["profiles\\2\\button_a"] == f"button:1,{G}" and controls["profiles\\2\\button_a\\default"] == "false"
    assert controls["profiles\\2\\button_home"] == "[empty]"
    assert controls["profiles\\1\\button_a"] == "code:65,engine:keyboard"
    assert controls["profiles\\2\\button_debug"] == "code:79,engine:keyboard" and controls["profiles\\size"] == "2"
    assert 'profiles\\2\\button_a="button:1,' in new and "profiles\\2\\button_home=[empty]" in new
    assert ini_section(new, "UI") == {"fullscreen": "true"}


def test_rewrite_grows_the_profile_list_when_needed():
    new = _azahar.rewrite("[Controls]\nprofile=0\n", {"button_a": _azahar.EMPTY})
    assert ini_section(new, "Controls")["profiles\\size"] == "1"


def test_plan_writes_azahars_config(tmp_path):
    (tmp_path / "azahar-emu").mkdir()
    (tmp_path / "azahar-emu" / "qt-config.ini").write_text(INI)
    assert list(_azahar.plan(Context([EDGE]))) == [tmp_path / "azahar-emu" / "qt-config.ini"]


def test_swapped_shoulders_exchange_l_r_with_zl_zr():
    got = _azahar.values_for(Context([EDGE], shoulders="swapped"))
    assert params(got["button_l"])["axis"] == "4" and got["button_zr"] == f"button:10,{G}"
