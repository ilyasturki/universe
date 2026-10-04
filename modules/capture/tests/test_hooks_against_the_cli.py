import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

MODULES = Path(__file__).resolve().parents[2]
CLI = os.environ.get("UNIVERSE_BIN", "")
SID = "20260911-120000"

pytestmark = pytest.mark.skipif(not os.path.isfile(CLI), reason="UNIVERSE_BIN names no built CLI: the hooks are checked against their shims alone")


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


capture = _load("capture_common_cli", MODULES / "capture" / "bin" / "_common.py")
journal = _load("journal_common_cli", MODULES / "journal" / "bin" / "_common.py")


@pytest.fixture
def game(tmp_path, monkeypatch):
    for kind in ("data", "config", "state", "cache"):
        monkeypatch.setenv(f"UNIVERSE_{kind.upper()}_HOME", str(tmp_path / kind))
        monkeypatch.setenv(f"XDG_{kind.upper()}_HOME", str(tmp_path / "xdg" / kind))
    for var, sub in (
        ("UNIVERSE_MODULES_PATH", "modules"),
        ("UNIVERSE_SOURCES_PATH", "sources"),
        ("HOME", "home"),
        ("XDG_DATA_DIRS", "share"),
        ("XDG_CONFIG_DIRS", "etc"),
        ("XDG_RUNTIME_DIR", "run"),
    ):
        monkeypatch.setenv(var, str(tmp_path / sub))
    monkeypatch.setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
    (tmp_path / "config").mkdir()
    (tmp_path / "config" / "config.toml").write_text('schema = 1\n[desktop]\nprofile = "none"\n')
    game = tmp_path / "data" / "games" / "testgame"
    game.mkdir(parents=True)
    (game / "game.toml").write_text('schema = 1\nid = "testgame"\ntitle = "Test Game: Redux"\n')
    return game


def test_capture_reads_the_clis_answers_with_no_screen_and_no_session(game, monkeypatch, capsys):
    mode = capture.cli_json(["screen-mode", "NOPE-9"])
    assert mode == {"screen": "NOPE-9", "width": 0, "height": 0, "refresh": 0, "vrr": False}, "test_capture's universe shim answers this shape"
    assert capture.screen_refresh_hz("NOPE-9") is None

    assert capture.cli_json(["session-window", "--wait", "0"]) is None
    assert "no session running" in capsys.readouterr().err, "the CLI took the call and found no session, not a usage error"

    replies = []
    run = subprocess.run
    monkeypatch.setattr(subprocess, "run", lambda *a, **kw: replies.append(run(*a, **kw)) or replies[-1])
    capture.show_osd("-1 frame")
    assert "no desktop profile" in replies[0].stderr, "the dash-led label got past the CLI's own parsing"


def test_a_journal_entry_the_hook_hands_over_is_one_the_core_lists(game, monkeypatch):
    shots = game / "screenshots"
    shots.mkdir()
    (shots / "20260911-120130.png").write_bytes(b"png")
    env = {
        **os.environ,
        "GAME_ID": "testgame",
        "GAME_TITLE": "Test Game: Redux",
        "SESSION_ID": SID,
        "SESSION_STARTED_AT": "2026-09-11T12:00:00+02:00",
        "SESSION_ENDED_AT": "2026-09-11T12:03:20+02:00",
        "SESSION_DURATION_S": "200",
        "JOURNAL_DIR": str(game / "journal"),
        "SCREENSHOTS_DIR": str(shots),
        "MODULE_DATA_DIR": str(game.parents[2] / "module"),
        "MODULE_SETTINGS_JSON": json.dumps({"provider": "stub"}),
        "JOURNAL_CORE_BACKOFF_S": "0",
    }
    res = subprocess.run([sys.executable, str(MODULES / "journal" / "bin" / "process")], env=env, capture_output=True, text=True, check=False)
    assert res.returncode == 0 and "handed to the core" in res.stderr, res.stderr
    assert sorted(p.name for p in (game / "journal").glob(f"{SID}.*")) == [f"{SID}.json"], "the core wrote it and the hook's pending file is gone"
    listed = json.loads(subprocess.run([CLI, "journal", "testgame", "--json"], capture_output=True, text=True, check=True).stdout)
    assert [(e["session"], e["state"], e["provider"], e["images"]) for e in listed] == [(SID, "written", "stub", [str(shots / "20260911-120130.png")])]

    monkeypatch.setattr(journal, "CORE_BACKOFF_S", 0)
    assert journal.add_entry_via_core(SID, json.dumps({"title": 1})) == "invalid", "test_journal's shim says `universe: invalid:` as the CLI does"
