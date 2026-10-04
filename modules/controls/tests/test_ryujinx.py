import json

import _ryujinx
import pytest
from _controls import Context, Skip
from controls_fixtures import EDGE, SWITCH_PRO, XBOX, twin

OLD_P1 = {
    "deadzone_left": 0.2,
    "range_left": 1.0,
    "trigger_threshold": 0.3,
    "rumble": {"strong_rumble": 1, "weak_rumble": 1, "enable_rumble": True, "use_hdrumble": False},
    "motion": {"motion_backend": "GamepadDriver", "sensitivity": 120, "gyro_deadzone": 1, "enable_motion": False},
    "left_joycon_stick": {"joystick": "Right", "invert_stick_x": True, "invert_stick_y": False, "rotate90_cw": False, "stick_button": "Guide"},
    "id": "0-00000003-054c-0000-e60c-000000016800",
    "player_index": "Player1",
}


def config(**extra):
    return {"version": 73, "docked_mode": True, "input_config": [OLD_P1, {"id": "x", "player_index": "Player3"}], "player_input_assignments": [], **extra}


@pytest.fixture
def base(tmp_path):
    root = tmp_path / "Ryujinx"
    root.mkdir()
    (root / "Config.json").write_text(json.dumps(config(), indent=2))
    return root


def test_ids_are_ryujinxs_guid_text_with_a_count_per_model():
    usb_edge = bytes.fromhex("0300e0274c050000f20d000000006800")
    assert _ryujinx.ryujinx_guid(usb_edge) == "00000003-054c-0000-f20d-000000006800"
    assert _ryujinx.ids([EDGE, XBOX, twin(EDGE)]) == [
        "0-00000005-054c-0000-f20d-000000006800",
        "0-00000005-045e-0000-8e02-000030110000",
        "1-00000005-054c-0000-f20d-000000006800",
    ]


@pytest.mark.parametrize(("pad", "abxy"), [(EDGE, ("B", "A", "Y", "X")), (XBOX, ("B", "A", "Y", "X")), (SWITCH_PRO, ("A", "B", "X", "Y"))])
def test_face_tokens_name_the_label_on_the_chosen_button(pad, abxy):
    doc = _ryujinx.rewrite({"version": 73}, Context([pad]))
    right = doc["input_config"][0]["right_joycon"]
    assert (right["button_a"], right["button_b"], right["button_x"], right["button_y"]) == abxy


def test_players_follow_the_pads_and_keep_their_tuning():
    doc = _ryujinx.rewrite(config(), Context([EDGE, XBOX]))
    p1, p2 = doc["input_config"]
    assert [p1["player_index"], p2["player_index"]] == ["Player1", "Player2"]
    assert p1["id"] == "0-00000005-054c-0000-f20d-000000006800" and p1["name"] == "DualSense Edge Wireless Controller (0)"
    assert p1["controller_type"] == "ProController" and p1["backend"] == "GamepadSDL3"
    left, right = p1["left_joycon"], p1["right_joycon"]
    assert (left["button_l"], left["button_zl"], right["button_r"], right["button_zr"]) == ("LeftShoulder", "LeftTrigger", "RightShoulder", "RightTrigger")
    assert (p1["deadzone_left"], p1["range_left"], p1["trigger_threshold"], p1["rumble"]["enable_rumble"]) == (0.2, 1.0, 0.3, True)
    assert p1["motion"]["sensitivity"] == 120 and p1["motion"]["enable_motion"] is True
    assert p1["left_joycon_stick"] == {"joystick": "Left", "invert_stick_x": True, "invert_stick_y": False, "rotate90_cw": False, "stick_button": "LeftStick"}
    assert p2["deadzone_left"] == 0.1 and p2["motion"]["enable_motion"] is False and p2["rumble"]["enable_rumble"] is False
    assert doc["player_input_assignments"][1] == {
        "player_index": "Player2",
        "enable_dynamic_input_swap": False,
        "devices": [{"type": "Controller", "id": "0-00000005-045e-0000-8e02-000030110000", "profile_name": None}],
    }


def test_plan_writes_the_global_config_and_the_games_that_carry_their_own_input(base):
    games = base / "games"
    for title, own in (("0100000000010000", False), ("010012101468c000", None), ("0100453019aa8000", True)):
        (games / title).mkdir(parents=True)
        (games / title / "Config.json").write_text(json.dumps(config(use_input_global_config=own), indent=2))
    (games / "broken").mkdir()
    (games / "broken" / "Config.json").write_text("{not json")
    out = _ryujinx.plan(Context([EDGE]))
    assert sorted(p.relative_to(base).as_posix() for p in out) == [
        "Config.json",
        "games/0100000000010000/Config.json",
        "games/010012101468c000/Config.json",
    ]
    doc = json.loads(out[base / "Config.json"])
    assert list(doc) == ["version", "docked_mode", "input_config", "player_input_assignments"]
    assert [e["player_index"] for e in doc["input_config"]] == ["Player1"]
    assert not out[base / "Config.json"].endswith("\n")


def test_plan_skips_without_a_readable_config(base):
    (base / "Config.json").write_text('{"version": "x"}')
    with pytest.raises(Skip):
        _ryujinx.plan(Context([EDGE]))
