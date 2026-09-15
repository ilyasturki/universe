import pytest

from pathlib import Path

from conftest import pump, wait_for
from universe_ui.universe_client import _json


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

    # a pending entry is cancelled by dropping its state file; a recording line is cleared even
    # when the file is already gone. Neither needs the trash.
    (games / "sample" / "journal" / "20260912-120000.pending.json").write_text(
        '{"session": "20260912-120000", "game": "sample", "started_at": "2026-09-12T12:00:00+02:00", "provider": "stub"}'
    )
    assert [e["state"] for e in client.journal("sample")] == ["pending", "written"]
    written.clear()
    assert client.removeJournalEntry("sample", "20260912-120000") is True
    assert [e["state"] for e in client.journal("sample")] == ["written"] and written == ["sample"]
    assert client.removeJournalEntry("sample", "20260912-120000") is False and seen[-1] == "NotFound"
    (games / "sample" / "sessions.jsonl").write_text(
        '{"session": "20260913-120000", "game": "sample", "started_at": "2026-09-13T12:00:00+02:00",'
        ' "ended_at": "2026-09-13T13:00:00+02:00", "duration_s": 3600, "source": "universe", "exit": 0,'
        ' "recording": "' + str(games.parent / "gone.mkv") + '"}\n'
    )
    client._core.reload_game("sample")
    assert [r["session"] for r in client.recordings("sample")] == ["20260913-120000"]
    assert client.removeRecording("sample", "20260913-120000") is True
    assert client.recordings("sample") == [] and client.game("sample")["stats"]["hours"] == 1.0

    runners = {r["id"]: r for r in client.runners()}
    assert runners["linux"]["kind"] == "linux" and "Nintendo Wii" in runners["dolphin"]["platforms"]
    assert client.setRunnerSetting("dolphin", "batch", "false") and runners != {r["id"]: r for r in client.runners()}
    assert client.setRunnerSetting("dolphin", "nope", "1") is False and seen[-1] == "Invalid"
    rom = games.parent / "F-Zero GX.iso"
    rom.write_bytes(b"")
    ident = client.addGame("yuzu", str(rom), "")
    assert ident == "f-zero-gx"
    game = client.game(ident)
    assert game["effective"]["runner"] == "eden" and game["platform"] == "Nintendo Switch"
    assert game["effective"]["options"]["fullscreen"] is True
    assert client.addGame("dolphin", str(rom), "F-Zero GX") == "" and seen[-1] == "Invalid"


def test_list_resolves_defaults(fake):
    rows = fake.list()
    assert [r["id"] for r in rows][:2] == ["the-technomancer", "mini-metro"]
    control = fake.game("control")
    assert "proton" not in control["launch"] and control["effective"]["proton"] == "proton-ge"
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
    assert fake.setRunnerSetting("dolphin", "nope", "1") is False and seen[-1][0] == "Invalid"
    assert fake.setRunnerSetting("nope", "exe", "/x") is False and seen[-1][0] == "NotFound"


def test_settings_merges_game_scope(fake):
    settings = fake.settings("the-technomancer")
    assert set(settings) == {"capture", "journal"}
    assert settings["journal"]["language"] == "fr"
    assert fake.settings("control")["capture"]["cursor"] is False
    assert fake.set("control", "tags", "a, b") is True
    assert fake.game("control")["tags"] == ["a", "b"]


def test_launch_runs_a_session(fake):
    started, launched = [], []
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
