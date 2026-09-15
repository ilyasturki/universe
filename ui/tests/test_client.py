"""CoreClient over a FakeCore in a tmp root: what the client derives from the core's files and marker."""

import json
from pathlib import Path

import pytest

from conftest import pump, wait_for


def _collect(signal):
    seen = []
    signal.connect(lambda *args: seen.append(args))
    return seen


def test_errors_are_signalled_not_raised(fake):
    seen = _collect(fake.error)
    assert fake.version() == "0.0.0-fake"
    assert fake.game("nope") == {}
    assert seen and seen[0][0] == "NotFound"
    assert fake.setSetting("capture", "", "codec", "mpeg2") is False
    assert seen[-1][0] == "Invalid"
    assert fake.setSetting("capture", "", "codec", "hevc") is True
    assert fake.getSettings("capture", "")["codec"] == "hevc"
    assert fake.setRunnerSetting("dolphin", "nope", "1") is False and seen[-1][0] == "Invalid"
    assert fake.setRunnerSetting("nope", "exe", "/x") is False and seen[-1][0] == "NotFound"
    assert fake.removeJournalEntry("control", "nope") is False and seen[-1] == ("NotFound", "journal entry nope")


def test_settings_merges_game_scope(fake):
    settings = fake.settings("the-technomancer")
    assert set(settings) == {"capture", "journal"}
    assert settings["journal"]["language"] == "fr"
    assert fake.settings("control")["capture"]["cursor"] is False
    assert fake.set("control", "tags", "a, b") is True
    assert fake.game("control")["tags"] == ["a", "b"]


def test_launch_writes_the_marker_and_the_end_comes_from_the_state_watch(fake):
    core = fake.core
    started, launched, ended = _collect(fake.sessionStarted), _collect(fake.launched), _collect(fake.sessionEnded)
    current = _collect(fake.currentSessionChanged)
    assert fake.currentSession is None
    fake.launch("control", "DP-1")
    assert wait_for(fake.launched, 3000) is not None
    marker = json.loads((core._root / "state" / "current-session.json").read_text())
    assert marker["id"] == "control" and marker["screen"] == "DP-1" and marker["session_id"] == launched[0][0]
    assert fake.currentSession["id"] == "control" and fake.currentSession["screen"] == "DP-1"
    assert started == [(launched[0][0], "control")] and len(current) == 1

    busy = _collect(fake.launchFailed)
    fake.launch("mini-metro", "DP-1")
    assert wait_for(fake.launchFailed, 3000) is not None
    assert busy[0][0] == "mini-metro" and "running" in busy[0][1]
    assert wait_for(fake.sessionShown, 3000) == (launched[0][0], True)

    # The fake ends the session after 2 s: the session line lands, then the marker goes.
    assert wait_for(fake.sessionEnded, 6000) is not None
    assert ended == [(launched[0][0], "control", ended[0][2])] and ended[0][2] >= 1
    assert fake.currentSession is None and len(current) == 2
    assert not (core._root / "state" / "current-session.json").exists()
    line = json.loads((core._root / "data" / "games" / "control" / "sessions.jsonl").read_text().splitlines()[-1])
    assert line["session"] == launched[0][0] and line["duration_s"] == ended[0][2]
    assert fake.game("control")["stats"]["play_count"] == 9
    assert [e["state"] for e in fake.journal("control")][:1] == ["pending"]


def test_a_stop_ends_the_session_now(fake):
    fake.launch("control", "")
    assert wait_for(fake.launched, 3000) is not None
    fake.stop("")
    args = wait_for(fake.sessionEnded, 3000)
    assert args is not None and args[1] == "control"
    assert fake.currentSession is None


def test_files_written_by_others_reach_the_screens(fake):
    core = fake.core
    changed, media, written, filed = _collect(fake.libraryChanged), _collect(fake.mediaChanged), _collect(fake.entryWritten), _collect(fake.recordingFiled)
    games = core._root / "data" / "games"
    (games / "control" / "media" / "extra.png").write_bytes(b"")
    for _ in range(30):
        pump(100)
        if changed:
            break
    assert changed == [(["control"],)] and written == [("", "control")] and filed == [("", "control", "")]
    assert media == [], "a file under media/ is a library change; mediaChanged follows the client's own picks"

    changed.clear()
    (games / "control" / "journal" / "20260914-120000.json").write_text('{"session": "20260914-120000", "game": "control"}')
    for _ in range(30):
        pump(100)
        if changed:
            break
    assert changed == [(["control"],)]

    # A pick through the client: mediaChanged at once, and the overrides watch sees the file too.
    assert fake.mediaSetSlot("control", "banner", fake.game("control")["media"]["logo"])
    assert media == [("control",)]
    assert fake.game("control")["media"]["banner"].startswith(str(core._root / "overrides" / "control"))
    assert fake.mediaUnset("control", "banner") is True and fake.mediaUnset("control", "banner") is False
    assert media == [("control",), ("control",)]


def test_install_job_reports_progress(fake):
    steps = _collect(fake.progress)
    job = fake.install("gog", "1207658930")
    assert job.startswith("job-")
    assert fake.jobs()[0]["id"] == job and fake.jobs()[0]["finished"] is False
    args = wait_for(fake.jobFinished, 10000)
    assert args is not None and args[0] == job and args[1] is True
    assert steps and steps[-1][1:3] == (20, 20)
    assert next(g for g in fake.sourceLibrary("gog") if g["id"] == "1207658930")["installed"] is True


def test_a_failing_job_reports_its_end(fake):
    def broken(source, game_id, progress=None):
        raise RuntimeError("offline")

    fake.core.install = broken
    job = fake.install("gog", "1")
    args = wait_for(fake.jobFinished, 3000)
    assert args == (job, False, "offline")


def test_the_real_core_reads_writes_and_watches(app):
    universe_core = pytest.importorskip("universe_core")
    from universe_ui.universe_client import CoreClient

    core = universe_core.Core()
    games = Path(core.data_home()) / "games"
    (games / "sample").mkdir(parents=True, exist_ok=True)
    (games / "sample" / "game.toml").write_text('schema = 1\nid = "sample"\ntitle = "Sample"\n')
    core.reload_game("sample")
    client = CoreClient(core)
    assert [g["id"] for g in client.list()] == ["sample"]
    assert client.currentSession is None and client.version() == core.version()

    seen = _collect(client.error)
    assert client.game("nope") == {} and seen == [("NotFound", "nope")]
    assert client.set("sample", "favorite", "true") and client.game("sample")["favorite"] is True

    # another process writes a journal entry: the watch reloads the game and tells the screens
    written = _collect(client.entryWritten)
    (games / "sample" / "journal").mkdir()
    pump(700)  # the new directory is itself a change; the watch on it starts here
    written.clear()
    (games / "sample" / "journal" / "20260911-120000.json").write_text(
        '{"session": "20260911-120000", "game": "sample", "written_at": "2026-09-11T12:10:00+02:00", "lang": "en",'
        ' "title": "First", "provider": "stub", "paragraphs": ["p"], "next_up": "", "images": []}'
    )
    for _ in range(30):
        pump(100)
        if written:
            break
    assert written == [("", "sample")]
    assert [e["title"] for e in client.journal("sample")] == ["First"]
    assert client.removeJournalEntry("sample", "20260912-120000") is False and seen[-1][0] == "NotFound"
    (games / "sample" / "sessions.jsonl").write_text(
        '{"session": "20260913-120000", "game": "sample", "started_at": "2026-09-13T12:00:00+02:00",'
        ' "ended_at": "2026-09-13T13:00:00+02:00", "duration_s": 3600, "source": "universe", "exit": 0,'
        ' "recording": "' + str(games.parent / "gone.mkv") + '"}\n'
    )
    core.reload_game("sample")
    assert [r["session"] for r in client.recordings("sample")] == ["20260913-120000"]
    row = client.sessions("")[0]
    assert (row["title"], row["journal"], row["recording"]["exists"], row["recording"]["duration_s"]) == ("Sample", None, False, 0)
    assert client.removeRecording("sample", "20260913-120000") is True
    assert client.recordings("sample") == [] and client.game("sample")["stats"]["hours"] == 1.0

    runners = {r["id"]: r for r in client.runners()}
    assert runners["linux"]["kind"] == "linux" and "Nintendo Wii" in runners["dolphin"]["platforms"]
    assert client.setRunnerSetting("dolphin", "batch", "false") and runners != {r["id"]: r for r in client.runners()}
    rom = games.parent / "F-Zero GX.iso"
    rom.write_bytes(b"")
    ident = client.addGame("yuzu", str(rom), "")
    assert ident == "f-zero-gx" and client.game(ident)["effective"]["runner"] == "eden"
    assert client.addGame("dolphin", str(rom), "F-Zero GX") == "" and seen[-1][0] == "Invalid"
    assert client.controllerSetButton("dualsense-edge", "south", "[]") and client.controllerSetButton("dualsense-edge", "south", "null")
    assert client.controllerBind('{"family": "*", "button": "south", "trigger": "press", "action": "nope"}') is False and seen[-1][0] == "Invalid"
    client.shutdown()


def test_the_launch_keys_fixture_is_the_cores_table(app):
    universe_core = pytest.importorskip("universe_core")
    from universe_ui.fake_core import LAUNCH_KEYS, FakeCore

    core = universe_core.Core()
    table = core.launch_keys("both", None)
    assert json.loads(LAUNCH_KEYS.read_text()) == table, "regenerate with `universe launch-keys --json > ui/universe_ui/fixtures/launch_keys.json`"
    screen = {"screen": "DP-1", "width": 2560, "height": 1440, "refresh": 144}
    assert FakeCore().launch_keys("game", screen) == core.launch_keys("game", screen), "the fake sizes the choices as the core does"
    assert FakeCore().launch_keys("global", {}) == core.launch_keys("global", {})
