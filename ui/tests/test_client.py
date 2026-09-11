import pytest

from pathlib import Path

from conftest import pump, wait_for
from universe_ui.universe_client import UniverseError, _json


def test_json_tolerance():
    assert _json("", []) == []
    assert _json(None, {}) == {}
    assert _json("not json", {"d": 1}) == {"d": 1}
    assert _json('{"a": [1]}', None) == {"a": [1]}
    assert _json(42, None) == 42


def test_core_client_reads_writes_and_watches(app):
    universe_core = pytest.importorskip("universe_core")
    from universe_ui.universe_client import CoreClient

    games = Path(universe_core.data_home()) / "games"
    (games / "sample").mkdir(parents=True, exist_ok=True)
    (games / "sample" / "game.toml").write_text('schema = 1\nid = "sample"\ntitle = "Sample"\n')
    client = CoreClient()
    assert [g["id"] for g in client.list()] == ["sample"]
    assert client.currentSession is None

    seen = []
    client.error.connect(lambda kind, message: seen.append(kind))
    assert client.game("nope") == {} and seen == ["NotFound"]
    assert client.set("sample", "favorite", "true") and client.game("sample")["favorite"] is True

    # another process writes a journal entry: the watch reloads the game and tells the screens
    written = []
    client.entryWritten.connect(lambda session_id, ident: written.append(ident))
    (games / "sample" / "journal").mkdir()
    pump(700)  # the new directory is itself a change; the watch on it starts here
    written.clear()
    (games / "sample" / "journal" / "20260911-120000.json").write_text(
        '{"session": "20260911-120000", "game": "sample", "written_at": "2026-09-11T12:10:00+02:00", "lang": "en",'
        ' "title": "First", "provider": "stub", "paragraphs": ["p"], "next_up": "", "images": []}'
    )
    for _ in range(30):
        pump(100)
        if "sample" in written:
            break
    assert "sample" in written
    assert [e["title"] for e in client.journal("sample")] == ["First"]


def test_list_resolves_defaults(fake):
    rows = fake.list()
    assert [r["id"] for r in rows][:2] == ["the-technomancer", "mini-metro"]
    control = fake.game("control")
    assert control["launch"]["proton"] == "proton-ge"
    assert control["modules"]["capture"]["enabled"] is True
    assert control["stats"]["play_count"] == 8
    assert fake.game("mirrors-edge")["stats"]["last_played"] is None


def test_errors_are_signalled_not_raised(fake):
    seen = []
    fake.error.connect(lambda kind, message: seen.append((kind, message)))
    assert fake.game("nope") == {}
    assert seen and seen[0][0] == "NotFound"
    assert fake.setSetting("capture", "", "codec", "mpeg2") is False
    assert seen[-1][0] == "Invalid"
    assert fake.setSetting("capture", "", "codec", "hevc") is True
    assert fake.getSettings("capture", "")["codec"] == "hevc"


def test_settings_merges_game_scope(fake):
    settings = fake.settings("the-technomancer")
    assert set(settings) == {"capture", "journal"}
    assert settings["journal"]["language"] == "fr"
    assert fake.settings("control")["capture"]["cursor"] is False
    assert fake.set("control", "tags", "a, b") is True
    assert fake.game("control")["tags"] == ["a", "b"]


def test_launch_runs_a_session(fake):
    started, launched, ended = [], [], []
    fake.sessionStarted.connect(lambda sid, ident: started.append(ident))
    fake.launched.connect(lambda sid, ident: launched.append(ident))
    assert fake.currentSession is None
    fake.launch("control", "DP-1")
    assert launched == ["control"]
    assert fake.currentSession["id"] == "control"
    assert fake.currentSession["screen"] == "DP-1"

    busy = []
    fake.launchFailed.connect(lambda ident, message: busy.append(message))
    fake.launch("mini-metro", "DP-1")
    assert busy and "running" in busy[0]

    args = wait_for(fake.sessionEnded, 6000)
    assert args is not None and args[1] == "control" and args[2] >= 1
    assert started == ["control"]
    assert fake.currentSession is None
    assert fake.game("control")["stats"]["play_count"] == 9


def test_install_job_reports_progress(fake):
    steps = []
    fake.progress.connect(lambda job, done, total, message: steps.append((done, total)))
    job = fake.install("gog", "1207658930")
    assert job.startswith("job-")
    args = wait_for(fake.jobFinished, 10000)
    assert args is not None and args[0] == job and args[1] is True
    assert steps and steps[-1] == (20, 20)
    assert next(g for g in fake.sourceLibrary("gog") if g["id"] == "1207658930")["installed"] is True
