import xml.etree.ElementTree as ET

import _cemu
import pytest
from _controls import Context
from _gamepads import Pad
from controls_fixtures import EDGE, SWITCH_PRO, XBOX, twin

EXISTING = """<?xml version="1.0" encoding="UTF-8"?>
<emulated_controller>
	<type>Wii U GamePad</type>
	<profile>Dualsense BOTW</profile>
	<toggle_display>0</toggle_display>
	<controller>
		<api>SDLController</api>
		<uuid>0_030057564c050000e60c000000006800</uuid>
		<display_name>DualSense Wireless Controller</display_name>
		<motion>true</motion>
		<rumble>0.25</rumble>
		<axis>
			<deadzone>0.1</deadzone>
			<range>1</range>
		</axis>
		<mappings>
			<entry>
				<mapping>1</mapping>
				<button>0</button>
			</entry>
		</mappings>
	</controller>
</emulated_controller>
"""


@pytest.fixture
def cemu(tmp_path, monkeypatch):
    config = tmp_path / "config" / "Cemu"
    (config / "controllerProfiles").mkdir(parents=True)
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "config"))
    return config


def parse(text):
    root = ET.fromstring(text)
    controller = root.find("controller")
    maps = {int(e.findtext("mapping")): int(e.findtext("button")) for e in controller.find("mappings")}
    return root, controller, maps


def test_uuids_count_pads_with_the_same_full_guid():
    assert _cemu.uuids([EDGE, XBOX, twin(EDGE)]) == [f"0_{EDGE.guid.hex()}", f"0_{XBOX.guid.hex()}", f"1_{EDGE.guid.hex()}"]


def test_positional_gamepad_is_cemus_own_default_without_home(cemu):
    files = _cemu.plan(Context([EDGE]))
    root, controller, maps = parse(files[cemu / "controllerProfiles" / "controller0.xml"])
    assert root.findtext("type") == "Wii U GamePad" and root.findtext("toggle_display") == "0"
    assert controller.findtext("api") == "SDLController" and controller.findtext("uuid") == f"0_{EDGE.guid.hex()}"
    assert controller.findtext("display_name") == EDGE.name and controller.findtext("motion") == "true"
    assert [maps[i] for i in (1, 2, 3, 4)] == [1, 0, 3, 2]
    assert (maps[5], maps[7], maps[8], maps[9], maps[10], maps[17], maps[20]) == (9, 42, 43, 6, 4, 45, 38)
    assert 27 not in maps and not set(maps.values()) & {5, 15, 16, 17, 18, 19, 20}


def test_second_player_is_a_pro_controller_with_xbox_letters_and_home(cemu):
    files = _cemu.plan(Context([EDGE, XBOX], "xbox", guide=True))
    root, controller, maps = parse(files[cemu / "controllerProfiles" / "controller1.xml"])
    assert root.findtext("type") == "Wii U Pro Controller" and root.find("toggle_display") is None
    assert controller.find("motion") is None
    assert [maps[i] for i in (1, 2, 3, 4)] == [0, 1, 2, 3]
    assert maps[11] == 5 and (maps[12], maps[16], maps[18], maps[25]) == (11, 7, 45, 40)


def test_a_nintendo_labelled_pad_is_swapped_back_for_sdl2(cemu):
    _, _, maps = parse(_cemu.plan(Context([SWITCH_PRO], "positional"))[cemu / "controllerProfiles" / "controller0.xml"])
    assert [maps[i] for i in (1, 2, 3, 4)] == [0, 1, 2, 3]


def test_an_existing_file_keeps_its_type_name_and_tuning(cemu):
    path = cemu / "controllerProfiles" / "controller0.xml"
    path.write_text(EXISTING.replace("Wii U GamePad", "Wii U Pro Controller"))
    root, controller, maps = parse(_cemu.plan(Context([XBOX]))[path])
    assert root.findtext("type") == "Wii U Pro Controller" and root.findtext("profile") == "Dualsense BOTW"
    assert controller.findtext("rumble") == "0.25" and controller.find("axis").findtext("deadzone") == "0.1"
    assert controller.findtext("uuid") == f"0_{XBOX.guid.hex()}" and maps[1] == 1


def test_only_gamepad_type_devices_are_players(cemu):
    stick = Pad(**{**vars(XBOX), "gamepad_type": False})
    files = _cemu.plan(Context([stick, EDGE]))
    assert list(files) == [cemu / "controllerProfiles" / "controller0.xml"]


def test_a_games_profile_follows_the_held_pad_and_keeps_its_mapping(cemu):
    (cemu / "gameProfiles").mkdir()
    (cemu / "gameProfiles" / "00050000101c9500.ini").write_text("[Graphics]\naccurateShaderMul = true\n[Controller]\ncontroller1 = Dualsense BOTW\n")
    named = cemu / "controllerProfiles" / "Dualsense BOTW.xml"
    named.write_text(EXISTING)
    text = _cemu.plan(Context([XBOX]))[named]
    _, controller, maps = parse(text)
    assert controller.findtext("uuid") == f"0_{XBOX.guid.hex()}" and controller.findtext("display_name") == XBOX.name
    assert maps == {1: 0} and controller.findtext("rumble") == "0.25"
    assert text.replace(f"0_{XBOX.guid.hex()}", "0_030057564c050000e60c000000006800").replace(XBOX.name, "DualSense Wireless Controller") == EXISTING


def test_swapped_shoulders_exchange_l_r_with_zl_zr(cemu):
    _, _, maps = parse(_cemu.plan(Context([EDGE], shoulders="swapped"))[cemu / "controllerProfiles" / "controller0.xml"])
    assert (maps[5], maps[6], maps[7], maps[8], maps[9]) == (42, 43, 9, 10, 6)
