import json

import pytest
from _controls import ini_section
from controls_fixtures import EDGE, script

INI = """[Controls]
player_0_button_a\\default=true
player_0_button_a="engine:keyboard,code:67,toggle:0"
"""


@pytest.fixture
def hook(tmp_path, monkeypatch):
    pre = script("pre")
    config = tmp_path / "config"
    (config / "eden").mkdir(parents=True)
    (config / "eden" / "qt-config.ini").write_text(INI)
    env_file = tmp_path / "env"
    env_file.write_text("")
    monkeypatch.setenv("XDG_CONFIG_HOME", str(config))
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "data"))
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path / "run"))
    monkeypatch.setenv("UNIVERSE_ENV_FILE", str(env_file))
    monkeypatch.setenv("SESSION_ID", "s1")
    monkeypatch.setenv("GAME_ID", "mk8")
    monkeypatch.setenv("MODULE_SETTINGS_JSON", json.dumps({"layout": "xbox"}))
    monkeypatch.setattr(pre, "pads_module_runs", lambda _game: False)
    monkeypatch.setattr(pre._gamepads, "pads", lambda _hints: [EDGE])

    def run(effective):
        monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps({"effective": effective}))
        return pre.main()

    run.ini = config / "eden" / "qt-config.ini"
    run.env = env_file
    run.mappings = tmp_path / "run" / "universe" / "controls-s1.txt"
    run.pre = pre
    return run


EDEN = {"runner": "eden", "runner_kind": "emulator"}


def test_pre_writes_edens_controls_and_backs_the_file_up_once(hook):
    assert hook(EDEN) == 0
    controls = ini_section(hook.ini.read_text(), "Controls")
    assert controls["player_0_button_a"].endswith(",button:0")
    assert controls["player_1_connected"] == "false"
    assert hook.ini.with_name("qt-config.ini.before-universe").read_text() == INI


def test_every_emulator_gets_the_pads_without_universes_buttons(hook):
    assert hook({**EDEN, "runner": "shadps4"}) == 0
    assert hook.env.read_text() == f"SDL_GAMECONTROLLERCONFIG_FILE={hook.mappings}\n"
    line = hook.mappings.read_text().strip()
    assert line.startswith(EDGE.guid.hex()) and "guide" not in line and "paddle1" not in line
    assert hook.ini.read_text() == INI


def test_an_emulators_own_database_goes_to_sdl_and_under_the_mapping(hook, monkeypatch):
    db = hook.ini.parents[1] / "rpcs3" / "input_configs" / "gamecontrollerdb.txt"
    db.parent.mkdir(parents=True)
    db.write_text("# rpcs3\n")
    seen = []
    monkeypatch.setattr(hook.pre._gamepads, "pads", lambda hints: seen.append(hints) or [EDGE])
    assert hook({**EDEN, "runner": "rpcs3"}) == 0
    assert seen == [{"SDL_GAMECONTROLLERCONFIG_FILE": str(db), "SDL_JOYSTICK_HIDAPI_PS3": "1"}]
    assert hook.mappings.read_text().startswith("# rpcs3\n")


def test_writes_follow_a_symlink_to_its_target(hook, tmp_path):
    real = tmp_path / "dotfiles" / "qt-config.ini"
    real.parent.mkdir()
    real.write_text(INI)
    hook.ini.unlink()
    hook.ini.symlink_to(real)
    assert hook(EDEN) == 0
    assert hook.ini.is_symlink() and "player_0_connected=true" in real.read_text()


@pytest.mark.parametrize(
    "effective",
    [
        {**EDEN, "runner": "proton", "runner_kind": "proton"},
        {**EDEN, "runner": "xenia"},
    ],
)
def test_pre_leaves_everything_alone_where_it_does_not_apply(hook, effective):
    assert hook(effective) == 0
    assert hook.ini.read_text() == INI and hook.env.read_text() == "" and not hook.mappings.exists()


def test_pre_stands_down_for_the_pads_module_and_without_pads(hook, monkeypatch):
    monkeypatch.setattr(hook.pre, "pads_module_runs", lambda _game: True)
    assert hook(EDEN) == 0 and hook.ini.read_text() == INI
    monkeypatch.setattr(hook.pre, "pads_module_runs", lambda _game: False)
    monkeypatch.setattr(hook.pre._gamepads, "pads", lambda _hints: [])
    assert hook(EDEN) == 0 and hook.ini.read_text() == INI and hook.env.read_text() == ""


def test_the_shoulders_setting_reaches_the_writer(hook, monkeypatch):
    monkeypatch.setenv("MODULE_SETTINGS_JSON", json.dumps({"shoulders": "swapped"}))
    assert hook(EDEN) == 0
    assert ",axis:4," in ini_section(hook.ini.read_text(), "Controls")["player_0_button_l"]
