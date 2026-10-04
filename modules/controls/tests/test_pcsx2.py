import _pcsx2
import pytest
from _controls import Context, section_values
from controls_fixtures import EDGE, SWITCH_PRO, XBOX

PCSX2_INI = """[Pad]
MultitapPort1 = false

[Pad1]
Type = DualShock2
Cross = Keyboard/Z
Cross = SDL-3/FaceEast

[Pad2]
Type = None

[Hotkeys]
OpenPauseMenu = SDL-0/Guide

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


def test_pcsx2_binds_the_held_pads_positionally_over_their_old_bindings(pcsx2):
    text = _pcsx2.plan(Context([EDGE, XBOX]))[pcsx2]
    pad1 = lines_of(text, "Pad1")
    assert pad1.count("Cross = SDL-0/FaceSouth") == 1 and not any(line.startswith("Cross = Keyboard") for line in pad1)
    assert {"Triangle = SDL-0/FaceNorth", "L1 = SDL-0/LeftShoulder", "L2 = SDL-0/+LeftTrigger", "LUp = SDL-0/-LeftY"} <= set(pad1)
    assert pad1.count("Type = DualShock2") == 1 and not any(line.startswith("Analog") for line in pad1)
    pad2 = lines_of(text, "Pad2")
    assert "Type = DualShock2" in pad2 and "Type = None" not in pad2 and "Cross = SDL-1/FaceSouth" in pad2


def test_a_third_pad_needs_the_users_multitap(pcsx2):
    assert "[Pad3]" not in _pcsx2.plan(Context([EDGE, XBOX, SWITCH_PRO]))[pcsx2]
    pcsx2.write_text(PCSX2_INI.replace("MultitapPort1 = false", "MultitapPort1 = true"))
    assert "Cross = SDL-2/FaceSouth" in lines_of(_pcsx2.plan(Context([EDGE, XBOX, SWITCH_PRO]))[pcsx2], "Pad3")


def test_guide_passed_through_toggles_analog(pcsx2):
    text = _pcsx2.plan(Context([EDGE], guide=True))[pcsx2]
    assert "Analog = SDL-0/Guide" in lines_of(text, "Pad1")
    assert "OpenPauseMenu = SDL-0/Guide" in lines_of(text, "Hotkeys")


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


def test_section_values_takes_the_last_of_a_repeated_key():
    assert section_values(PCSX2_INI, "Pad1")["Cross"] == "SDL-3/FaceEast"
