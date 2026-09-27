import _dolphin
import pytest
from _controls import Context, ini_section
from controls_fixtures import EDGE, SWITCH_PRO, XBOX, twin

DOLPHIN_INI = """[Core]
SIDevice0 = 6
SIDevice1 = 0
SIDevice2 = 0
SIDevice3 = 0
[SDL_Hints]
SDL_JOYSTICK_HIDAPI_PS5_PLAYER_LED = 1
"""
GCPAD_INI = """[GCPad1]
Device = SDL/0/DualSense Wireless Controller
Buttons/A = `Button E`
Buttons/Z = `Trigger L`+`Trigger R`
Main Stick/Calibration = 90.00 120.00
Main Stick/Modifier/Range = 100.
Options/Always Connected = True
[GCPad2]
Device = XInput2/0/Virtual core pointer
"""
WIIMOTE_INI = """[Wiimote1]
Device = SDL/0/DualSense Wireless Controller
Buttons/A = `Button E`
Shake/Y = `Trigger L`
Extension = Classic
IMUIR/Total Yaw = 50.
Source = 1
[Wiimote2]
Device = XInput2/0/Virtual core pointer
Source = 0
[BalanceBoard]
Source = 0
"""


@pytest.fixture
def dolphin(tmp_path, monkeypatch):
    config = tmp_path / "config" / "dolphin-emu"
    games = tmp_path / "data" / "dolphin-emu" / "GameSettings"
    config.mkdir(parents=True)
    games.mkdir(parents=True)
    (config / "Dolphin.ini").write_text(DOLPHIN_INI)
    (config / "GCPadNew.ini").write_text(GCPAD_INI)
    (config / "WiimoteNew.ini").write_text(WIIMOTE_INI)
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "config"))
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "data"))
    return config, games


def run(ctx):
    return {p.name: t for p, t in _dolphin.plan(ctx).items()}


def test_hints_are_dolphins_defaults_under_its_own_sdl_hints(dolphin):
    hints = _dolphin.hints()
    assert hints["SDL_JOYSTICK_HIDAPI_PS5_PLAYER_LED"] == "1" and hints["SDL_JOYSTICK_ENHANCED_REPORTS"] == "1"
    assert hints["SDL_JOYSTICK_HIDAPI_GAMECUBE"] == "1"
    config, _ = dolphin
    (config / "Dolphin.ini").write_text("[Core]\nSIDevice2 = 12\n")
    assert _dolphin.hints()["SDL_JOYSTICK_HIDAPI_GAMECUBE"] == "0"


def test_devices_count_pads_of_the_same_name():
    assert _dolphin.devices([EDGE, XBOX, twin(EDGE)]) == [
        "SDL/0/DualSense Edge Wireless Controller",
        "SDL/0/Xbox Wireless Controller",
        "SDL/1/DualSense Edge Wireless Controller",
    ]


def test_gamecube_takes_dolphins_preset_and_keeps_the_users_settings(dolphin):
    gc = ini_section(run(Context([EDGE], "positional"))["GCPadNew.ini"], "GCPad1")
    assert gc["Device"] == "SDL/0/DualSense Edge Wireless Controller"
    assert (gc["Buttons/A"], gc["Buttons/B"], gc["Buttons/Z"]) == ("`Button S`", "`Button E`", "`Shoulder R`")
    assert gc["Triggers/L-Analog"] == "`Trigger L`" and gc["Main Stick/Up"] == "`Left Y+`"
    assert gc["Main Stick/Calibration"] == "90.00 120.00" and gc["Options/Always Connected"] == "True"
    assert gc["C-Stick/Calibration"] == _dolphin.CALIBRATION
    assert not any("Guide" in v or "Misc" in v or "Paddle" in v for v in gc.values())


def test_wiimote_keeps_its_extension_and_classic_follows_the_layout(dolphin):
    text = run(Context([EDGE], "xbox"))["WiimoteNew.ini"]
    wm = ini_section(text, "Wiimote1")
    assert wm["Extension"] == "Classic" and wm["IMUIR/Total Yaw"] == "50." and wm["Source"] == "1"
    assert wm["Buttons/A"] == "`Button S`" and wm["Buttons/B"] == "`Trigger R`"
    assert wm["Classic/Buttons/A"] == "`Button S`" and wm["Classic/Buttons/B"] == "`Button E`"
    assert wm["IMUGyroscope/Yaw Left"] == "`Gyro Yaw Left`" and "IR/Up" not in wm
    assert "Buttons/Home" not in wm and "Shake/Y" in wm and wm["Shake/Y"] == "`Shoulder R`"
    assert ini_section(text, "BalanceBoard") == {"Source": "0"}
    positional = ini_section(run(Context([EDGE], "positional"))["WiimoteNew.ini"], "Wiimote1")
    assert positional["Classic/Buttons/A"] == "`Button E`" and positional["Classic/Buttons/Y"] == "`Button W`"


def test_a_pad_without_gyro_points_with_the_right_stick(dolphin):
    wm = ini_section(run(Context([EDGE, XBOX], "positional", guide=True))["WiimoteNew.ini"], "Wiimote2")
    assert wm["Device"] == "SDL/0/Xbox Wireless Controller"
    assert wm["IR/Up"] == "`Right Y+`" and wm["IR/Relative Input"] == "True" and "IMUGyroscope/Yaw Left" not in wm
    assert wm["Extension"] == "Nunchuk" and wm["Buttons/Home"] == "Guide" and wm["Source"] == "1"


def test_ports_open_for_the_held_pads_only(dolphin):
    files = run(Context([EDGE, SWITCH_PRO]))
    core = ini_section(files["Dolphin.ini"], "Core")
    assert [core[f"SIDevice{n}"] for n in range(4)] == ["6", "6", "0", "0"]
    assert ini_section(files["Dolphin.ini"], "SDL_Hints") == {"SDL_JOYSTICK_HIDAPI_PS5_PLAYER_LED": "1"}
    assert "[GCPad3]" not in files["GCPadNew.ini"]


def test_a_wii_game_leaves_the_gamecube_ports(dolphin):
    files = run(Context([EDGE, XBOX], platform="Nintendo Wii"))
    assert ini_section(files["Dolphin.ini"], "Core")["SIDevice1"] == "0"
    assert ini_section(files["WiimoteNew.ini"], "Wiimote2")["Source"] == "1"


def test_a_games_own_profile_keeps_its_scheme_on_the_held_pad(dolphin):
    config, games = dolphin
    (games / "SMNP01.ini").write_text("[Controls]\nWiimoteProfile1 = sideways\nPadProfile2 = pad\n")
    (config / "Profiles" / "Wiimote").mkdir(parents=True)
    (config / "Profiles" / "GCPad").mkdir(parents=True)
    (config / "Profiles" / "Wiimote" / "sideways.ini").write_text(
        "[Profile]\nDevice = SDL/0/DualSense Wireless Controller\nOptions/Sideways Wiimote = True\nButtons/2 = `Button S`\n"
    )
    (config / "Profiles" / "GCPad" / "pad.ini").write_text("[Profile]\nDevice = SDL/0/DualSense Wireless Controller\n")
    files = _dolphin.plan(Context([XBOX]))
    profile = ini_section(files[config / "Profiles" / "Wiimote" / "sideways.ini"], "Profile")
    assert profile == {"Device": "SDL/0/Xbox Wireless Controller", "Options/Sideways Wiimote": "True", "Buttons/2": "`Button S`"}
    assert config / "Profiles" / "GCPad" / "pad.ini" not in files


def test_a_profile_on_a_keyboard_is_left_alone(dolphin):
    config, games = dolphin
    (games / "GFZP01.ini").write_text("[Controls]\nPadProfile1 = keys\n")
    (config / "Profiles" / "GCPad").mkdir(parents=True)
    (config / "Profiles" / "GCPad" / "keys.ini").write_text("[Profile]\nDevice = XInput2/0/Virtual core pointer\n")
    assert config / "Profiles" / "GCPad" / "keys.ini" not in _dolphin.plan(Context([EDGE]))


def test_userpath_holds_config_and_game_settings(tmp_path, monkeypatch):
    monkeypatch.setenv("DOLPHIN_EMU_USERPATH", str(tmp_path / "user"))
    assert _dolphin.user_dirs() == (tmp_path / "user" / "Config", tmp_path / "user" / "GameSettings")
