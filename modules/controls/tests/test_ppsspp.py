import _ppsspp
import pytest
from _controls import Context, Skip, section_values
from controls_fixtures import EDGE, SWITCH_PRO, XBOX

CONTROLS = """﻿[ControlMapping]
Up = 1-19,10-19
Circle = 1-52,10-190
Cross = 1-54,10-189
L = 1-45,10-194
Pause = 1-111,10-109,10-104,10-4034
Analog speed = 10-4036
Fast-forward = 1-61,1-59:10-198
Home = 12-4

[Other]
Keep = 10-189
"""


@pytest.fixture
def controls(tmp_path):
    path = tmp_path / "ppsspp" / "PSP" / "SYSTEM" / "controls.ini"
    path.parent.mkdir(parents=True)
    path.write_text(CONTROLS)
    return path


def test_every_pad_mapping_goes_and_the_held_pad_is_bound_at_its_sdl_index(controls):
    text = _ppsspp.plan(Context([XBOX, EDGE]))[controls]
    got = section_values(text, "ControlMapping")
    assert got["Cross"] == "1-54,11-189" and got["Circle"] == "1-52,11-190" and got["Up"] == "1-19,11-19"
    assert got["L"] == "1-45,11-193,11-4008" and got["R"] == "11-192,11-4010" and got["An.Up"] == "11-4003" and got["RightAn.Right"] == "11-4004"
    assert got["Pause"] == "1-111" and "Analog speed" not in got and got["Fast-forward"] == "1-61"
    assert got["Start"] == "11-197" and got["Select"] == "11-196"
    assert not any(part.split(":")[-1].split("-")[1] in ("198", "199", "200", "201", "202", "203") for v in got.values() for part in v.split(","))


def test_the_bom_and_other_sections_stay(controls):
    text = _ppsspp.plan(Context([EDGE]))[controls]
    assert text.startswith("﻿[ControlMapping]") and "[Other]\nKeep = 10-189" in text


def test_a_nintendo_pad_gets_cross_back_on_its_bottom_button(controls):
    got = section_values(_ppsspp.plan(Context([SWITCH_PRO]))[controls], "ControlMapping")
    assert (got["Cross"], got["Circle"], got["Square"], got["Triangle"]) == ("1-54,12-190", "1-52,12-189", "12-188", "12-191")


def test_home_goes_to_the_psps_home_so_it_never_opens_ppsspps_menu(controls):
    assert section_values(_ppsspp.plan(Context([EDGE]))[controls], "ControlMapping")["Home"] == "10-4"


def test_home_passed_through_is_left_unbound_for_ppsspps_menu(controls):
    got = section_values(_ppsspp.plan(Context([EDGE], guide=True))[controls], "ControlMapping")
    assert not any(part.endswith("-4") for v in got.values() for part in v.split(","))


def test_writing_twice_changes_nothing(controls):
    once = _ppsspp.plan(Context([EDGE]))[controls]
    controls.write_text(once)
    assert _ppsspp.plan(Context([EDGE]))[controls] == once


def test_a_file_without_the_section_is_not_given_a_partial_one(controls):
    controls.write_text("[Other]\nKeep = 1\n")
    with pytest.raises(Skip):
        _ppsspp.plan(Context([EDGE]))
