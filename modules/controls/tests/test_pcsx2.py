import _duckstation
import _pcsx2
import pytest
from _controls import Context, section_values
from controls_fixtures import EDGE, SWITCH_PRO, XBOX

PCSX2_INI = """[UI]
SettingsVersion = 1

[Pad]
MultitapPort1 = false

[Pad1]
Type = DualShock2
Cross = Keyboard/Z
Cross = SDL-3/FaceEast
Deadzone = 0.1
Macro1 = SDL-0/Paddle1
Macro1Binds = Cross

[Pad2]
Type = None

[Hotkeys]
OpenPauseMenu = SDL-0/Guide
Screenshot = Keyboard/F8
QuickSave = SDL-0/Back & SDL-0/Misc1
ToggleTurbo = SDL-0/Back & SDL-0/Start

[SDLHints]
SDL_JOYSTICK_HIDAPI_XBOX = 0
"""


def lines_of(text, name):
    body, current = [], None
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("["):
            current = s[1:-1]
        elif current == name and s:
            body.append(s)
    return body


@pytest.fixture
def pcsx2(tmp_path, monkeypatch):
    monkeypatch.setenv("PATH", str(tmp_path / "nothing"))
    ini = tmp_path / "PCSX2" / "inis" / "PCSX2.ini"
    ini.parent.mkdir(parents=True)
    ini.write_text(PCSX2_INI)
    return ini


def test_player_ids_take_the_sdl_player_index_else_the_lowest_free():
    assert _pcsx2.player_ids([EDGE, XBOX]) == [0, 1]
    unnumbered = [type(EDGE)(**{**vars(EDGE), "player_index": -1}), XBOX]
    assert _pcsx2.player_ids(unnumbered) == [0, 1]
    clash = [EDGE, type(XBOX)(**{**vars(XBOX), "player_index": 0})]
    assert _pcsx2.player_ids(clash) == [0, 1]


def test_pcsx2_binds_the_held_pads_positionally_and_keeps_the_rest(pcsx2):
    text = _pcsx2.plan(Context([EDGE, XBOX]))[pcsx2]
    pad1 = lines_of(text, "Pad1")
    assert pad1.count("Cross = SDL-0/FaceSouth") == 1 and not any(line.startswith("Cross = Keyboard") for line in pad1)
    assert "Triangle = SDL-0/FaceNorth" in pad1 and "L2 = SDL-0/+LeftTrigger" in pad1 and "LUp = SDL-0/-LeftY" in pad1
    assert "Deadzone = 0.1" in pad1 and "Macro1Binds = Cross" in pad1 and "Macro1 = SDL-0/Paddle1" not in pad1
    assert pad1.count("Type = DualShock2") == 1 and not any(line.startswith("Analog") for line in pad1)
    pad2 = lines_of(text, "Pad2")
    assert "Type = DualShock2" in pad2 and "Type = None" not in pad2 and "Cross = SDL-1/FaceSouth" in pad2
    hotkeys = lines_of(text, "Hotkeys")
    assert hotkeys == ["Screenshot = Keyboard/F8", "ToggleTurbo = SDL-0/Back & SDL-0/Start"]
    assert lines_of(text, "UI") == ["SettingsVersion = 1"]


def test_a_third_pad_needs_the_users_multitap(pcsx2):
    assert "[Pad3]" not in _pcsx2.plan(Context([EDGE, XBOX, SWITCH_PRO]))[pcsx2]
    pcsx2.write_text(PCSX2_INI.replace("MultitapPort1 = false", "MultitapPort1 = true"))
    assert "Cross = SDL-2/FaceSouth" in lines_of(_pcsx2.plan(Context([EDGE, XBOX, SWITCH_PRO]))[pcsx2], "Pad3")


def test_guide_passed_through_toggles_analog(pcsx2):
    text = _pcsx2.plan(Context([EDGE], guide=True))[pcsx2]
    assert "Analog = SDL-0/Guide" in lines_of(text, "Pad1")
    assert "OpenPauseMenu = SDL-0/Guide" in lines_of(text, "Hotkeys")


def test_writing_twice_changes_nothing(pcsx2):
    once = _pcsx2.plan(Context([EDGE, XBOX]))[pcsx2]
    pcsx2.write_text(once)
    assert _pcsx2.plan(Context([EDGE, XBOX]))[pcsx2] == once


def test_pcsx2_hints_and_database(pcsx2, tmp_path):
    hints = _pcsx2.hints()
    assert hints["SDL_JOYSTICK_HIDAPI_PS3"] == "1" and hints["SDL_JOYSTICK_ENHANCED_REPORTS"] == "auto"
    assert hints["SDL_JOYSTICK_HIDAPI_XBOX"] == "0"
    db = tmp_path / "PCSX2" / "game_controller_db.txt"
    db.write_text("")
    assert _pcsx2.databases() == [db]


def test_the_bundled_database_is_found_from_the_program(pcsx2, tmp_path, monkeypatch):
    store = tmp_path / "store"
    (store / "bin").mkdir(parents=True)
    (store / "bin" / "pcsx2-qt").write_text("#!/bin/sh\n")
    (store / "bin" / "pcsx2-qt").chmod(0o755)
    db = store / "share" / "PCSX2" / "resources" / "game_controller_db.txt"
    db.parent.mkdir(parents=True)
    db.write_text("")
    monkeypatch.setenv("PATH", str(store / "bin"))
    assert _pcsx2.databases() == [db]


DUCK_INI = """[Main]
SettingsVersion = 3

[ControllerPorts]
MultitapMode = Disabled

[Pad1]
Type = AnalogController
Cross = SDL-0/A
Analog = SDL-0/Guide
Up = SDL-5D3F-1234/DPadUp

[Hotkeys]
OpenPauseMenu = SDL-0/RightPaddle1

[InputSources]
SDLJoystickXboxHIDAPI = false
"""


@pytest.fixture
def duck(tmp_path, monkeypatch):
    monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
    ini = tmp_path / ".local/share/duckstation/settings.ini"
    ini.parent.mkdir(parents=True)
    ini.write_text(DUCK_INI)
    return ini


def test_duckstation_writes_sdls_positional_names(duck):
    text = _duckstation.plan(Context([SWITCH_PRO, XBOX]))[duck]
    pad1 = lines_of(text, "Pad1")
    assert "Cross = SDL-2/A" in pad1 and "Circle = SDL-2/B" in pad1 and "Triangle = SDL-2/Y" in pad1
    assert not any("Guide" in line or "SDL-5D3F" in line for line in pad1)
    assert "Type = AnalogController" in lines_of(text, "Pad2") and "Cross = SDL-1/A" in lines_of(text, "Pad2")
    assert lines_of(text, "Hotkeys") == []


def test_duckstation_looks_in_an_absolute_xdg_config_home_first(duck, tmp_path, monkeypatch):
    xdg = tmp_path / "xdg"
    (xdg / "duckstation").mkdir(parents=True)
    (xdg / "duckstation" / "settings.ini").write_text(DUCK_INI)
    monkeypatch.setenv("XDG_CONFIG_HOME", str(xdg))
    assert _duckstation.config_path() == xdg / "duckstation" / "settings.ini"
    monkeypatch.setenv("XDG_CONFIG_HOME", "relative")
    assert _duckstation.config_path() == duck


def test_duckstation_multitap_mode_opens_more_ports(duck):
    duck.write_text(DUCK_INI.replace("MultitapMode = Disabled", "MultitapMode = Port1Only"))
    text = _duckstation.plan(Context([EDGE, XBOX, SWITCH_PRO]))[duck]
    assert "Cross = SDL-2/A" in lines_of(text, "Pad3")


def test_duckstation_hints_follow_its_input_sources(duck):
    hints = _duckstation.hints()
    assert hints["SDL_JOYSTICK_HIDAPI_XBOX"] == "0" and hints["SDL_JOYSTICK_LINUX_DIGITAL_HATS"] == "0"
    assert hints["SDL_JOYSTICK_ENHANCED_REPORTS"] == "0" and hints["SDL_JOYSTICK_HIDAPI_PS3"] == "1"


def test_section_values_takes_the_last_of_a_repeated_key():
    assert section_values(PCSX2_INI, "Pad1")["Cross"] == "SDL-3/FaceEast"


def test_swapped_shoulders_exchange_l1_r1_with_l2_r2(pcsx2):
    pad1 = lines_of(_pcsx2.plan(Context([EDGE], shoulders="swapped"))[pcsx2], "Pad1")
    assert "L1 = SDL-0/+LeftTrigger" in pad1 and "R2 = SDL-0/RightShoulder" in pad1 and "L3 = SDL-0/LeftStick" in pad1
