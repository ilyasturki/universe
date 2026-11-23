import ast
import inspect
from pathlib import Path

import pytest

import universe_core

STUB = Path(__file__).resolve().parents[1] / "typings" / "universe_core.pyi"


@pytest.fixture
def core(tmp_path, monkeypatch):
    for kind in ("data", "config", "state", "cache"):
        monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(tmp_path / kind))
    return universe_core.Core()


def stub_methods():
    tree = ast.parse(STUB.read_text())
    cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "Core")
    return {f.name: [a.arg for a in f.args.args[1:]] for f in cls.body if isinstance(f, ast.FunctionDef) and f.name != "__init__"}


def test_the_stub_lists_every_method_with_its_parameters():
    stub = stub_methods()
    assert set(stub) == {n for n in dir(universe_core.Core) if not n.startswith("_")}
    for name, params in stub.items():
        signature = inspect.signature(getattr(universe_core.Core, name))
        assert [p for p in signature.parameters if p != "self"] == params, name
    assert issubclass(universe_core.UniverseError, Exception)


def test_an_error_carries_its_kind_then_its_message(core):
    with pytest.raises(universe_core.UniverseError) as raised:
        core.get("nope")
    assert raised.value.args == ("NotFound", "nope")


def test_a_shape_the_core_refuses_is_invalid_before_anything_runs(core):
    with pytest.raises(universe_core.UniverseError) as raised:
        core.add_entry("20260911-120000", {"title": 1})
    assert raised.value.args[0] == "Invalid"


def test_launch_keys_takes_a_screen_dict_or_none(core):
    assert core.launch_keys("both", None)
    screen = {"screen": "DP-1", "width": 2560, "height": 1440, "refresh": 144}
    assert core.launch_keys("game", screen)
    with pytest.raises(universe_core.UniverseError) as raised:
        core.launch_keys("game", {"width": "wide"})
    assert raised.value.args[0] == "Invalid"


def test_the_profile_comes_from_the_environment(core, tmp_path):
    assert core.data_home() == str(tmp_path / "data")
    assert core.state_home() == str(tmp_path / "state")
    assert core.version().startswith("0.")


def test_a_launch_held_in_its_pre_launch_hook_is_called_off_from_another_thread(tmp_path, monkeypatch):
    import threading
    import time

    module = tmp_path / "modules" / "slow"
    (module / "bin").mkdir(parents=True)
    (module / "module.toml").write_text('api = 2\nid = "slow"\n[hooks]\npre-launch = "bin/pre"\n')
    entered = tmp_path / "entered"
    (module / "bin" / "pre").write_text(f"#!/bin/sh\ntouch {entered}\nsleep 30\n")
    (module / "bin" / "pre").chmod(0o755)
    for kind in ("data", "config", "state", "cache"):
        monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(tmp_path / kind))
    monkeypatch.setenv("UNIVERSE_MODULES_PATH", str(tmp_path / "modules"))
    monkeypatch.setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null/bus")
    (tmp_path / "config").mkdir()
    (tmp_path / "config" / "config.toml").write_text('schema = 1\n[desktop]\nprofile = "none"\n[modules]\nenabled = ["slow"]\n')
    game = tmp_path / "data" / "games" / "quest"
    game.mkdir(parents=True)
    (game / "game.toml").write_text(f'schema = 1\nid = "quest"\ntitle = "Quest"\n[launch]\nrunner = "linux"\nexe = "{tmp_path / "quest.sh"}"\n')
    core = universe_core.Core()
    assert core.cancel_launch() is False, "nothing preparing"
    errors = []
    launch = threading.Thread(target=lambda: errors.append(pytest.raises(universe_core.UniverseError, core.launch, "quest", "")))
    launch.start()
    deadline = time.monotonic() + 5
    while not entered.exists():
        assert time.monotonic() < deadline, "the hook never ran"
        time.sleep(0.01)
    assert core.cancel_launch() is True
    launch.join(3)
    assert not launch.is_alive(), "the launch returned"
    assert "called off" in str(errors[0].value)
    assert core.current() is None


def test_a_game_written_on_disk_reads_back(core):
    games = Path(core.data_home()) / "games"
    (games / "sample").mkdir(parents=True)
    (games / "sample" / "game.toml").write_text('schema = 1\nid = "sample"\ntitle = "Sample"\n')
    core.reload_game("sample")
    assert [g["id"] for g in core.list()] == ["sample"]
    assert core.resolve("samp") == ["sample"]
    core.set("sample", "favorite", "true")
    assert core.get("sample")["favorite"] is True
