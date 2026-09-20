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


def test_a_game_written_on_disk_reads_back(core):
    games = Path(core.data_home()) / "games"
    (games / "sample").mkdir(parents=True)
    (games / "sample" / "game.toml").write_text('schema = 1\nid = "sample"\ntitle = "Sample"\n')
    core.reload_game("sample")
    assert [g["id"] for g in core.list()] == ["sample"]
    assert core.resolve("samp") == ["sample"]
    core.set("sample", "favorite", "true")
    assert core.get("sample")["favorite"] is True
