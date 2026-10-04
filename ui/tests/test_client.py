import json
from pathlib import Path

import pytest
from PySide6.QtCore import QTimer
from uitest import record, until


def test_errors_are_signalled_not_raised(fake):
    seen = record(fake.error)
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
    assert set(settings) == {"capture", "journal", "screenshot", "controls"}
    assert settings["journal"]["language"] == "fr"
    assert fake.settings("control")["capture"]["cursor"] is False
    assert fake.set("control", "tags", "a, b") is True
    assert fake.game("control")["tags"] == ["a", "b"]


def test_a_session_end_tells_once_about_an_enabled_module_the_core_skipped(fake, monkeypatch):
    from universe_ui import universe_client

    monkeypatch.setattr(universe_client, "SKIPPED_NOTICE_MS", 100)
    next(m for m in fake.core._data["modules"] if m["id"] == "capture").update(available=False, missing=["gsr-cli"])
    notices, ended = record(fake.notice), record(fake.sessionEnded)
    fake.launch("control", "DP-1")
    until(lambda: fake.currentSession)
    fake.core.end_session()
    until(lambda: ended)
    assert notices == [], "after the session toast, not over it"
    assert until(lambda: notices) == [("Video capture was on but ran nothing: missing gsr-cli",)]
    fake.launch("control", "DP-1")
    until(lambda: fake.currentSession)
    fake.core.end_session()
    until(lambda: len(ended) == 2)
    told = []
    QTimer.singleShot(universe_client.SKIPPED_NOTICE_MS, lambda: told.append(len(notices)))
    assert until(lambda: told) == [1], "said once per run"


def test_launch_writes_the_marker_and_the_end_comes_from_the_state_watch(fake):
    core = fake.core
    started, launched, ended = record(fake.sessionStarted), record(fake.launched), record(fake.sessionEnded)
    current = record(fake.currentSessionChanged)
    assert fake.currentSession is None
    fake.launch("control", "DP-1")
    until(lambda: launched)
    marker = json.loads((core._root / "state" / "current-session.json").read_text())
    assert marker["id"] == "control" and marker["screen"] == "DP-1" and marker["session_id"] == launched[0][0]
    assert fake.currentSession["id"] == "control" and fake.currentSession["screen"] == "DP-1"
    assert started == [(launched[0][0], "control")] and len(current) == 1

    busy = record(fake.launchFailed)
    shown = record(fake.sessionShown)
    fake.launch("mini-metro", "DP-1")
    until(lambda: busy)
    assert busy[0][0] == "mini-metro" and "running" in busy[0][1]
    assert until(lambda: shown) == [(launched[0][0], True)]

    # The game quits: the session line lands, then the marker goes.
    fake.core.end_session()
    until(lambda: ended)
    assert ended == [(launched[0][0], "control", ended[0][2], "quit")] and ended[0][2] >= 1
    assert fake.currentSession is None and len(current) == 2
    assert not (core._root / "state" / "current-session.json").exists()
    line = json.loads((core._root / "data" / "games" / "control" / "sessions.jsonl").read_text().splitlines()[-1])
    assert line["session"] == launched[0][0] and line["duration_s"] == ended[0][2]
    assert fake.game("control")["stats"]["play_count"] == 9
    assert [e["state"] for e in fake.journal("control")][:1] == ["pending"]


def test_the_window_wait_holds_while_the_session_lives(fake):
    fake.core.window_misses = 2
    shown, launched = record(fake.sessionShown), record(fake.launched)
    fake.launch("control", "DP-1")
    until(lambda: launched)
    until(lambda: shown)
    assert shown == [(launched[0][0], True)] and fake.core.window_misses == 0


def test_a_stop_ends_the_session_now(fake):
    launched, ended = record(fake.launched), record(fake.sessionEnded)
    fake.launch("control", "")
    until(lambda: launched)
    fake.stop("")
    args = until(lambda: ended)[0]
    assert args[1] == "control" and args[3] == "stopped", "a stop is not a crash"
    assert fake.currentSession is None


def test_files_written_by_others_reach_the_screens(fake):
    core = fake.core
    changed, media, written, filed = record(fake.libraryChanged), record(fake.mediaChanged), record(fake.entryWritten), record(fake.recordingFiled)
    games = core._root / "data" / "games"
    (games / "control" / "media" / "extra.png").write_bytes(b"")
    until(lambda: changed)
    assert changed == [(["control"],)] and written == [("", "control")] and filed == [("", "control", "")]
    assert media == [], "a file under media/ is a library change; mediaChanged follows the client's own picks"

    changed.clear()
    (games / "control" / "journal" / "20260914-120000.json").write_text('{"session": "20260914-120000", "game": "control"}')
    until(lambda: changed)
    assert changed == [(["control"],)]

    changed.clear()
    (games / "the-technomancer" / "screenshots" / "20260914-120500.png").write_bytes(b"")
    assert until(lambda: changed) == [(["the-technomancer"],)], "a shot taken in-game lands in screenshots/"

    assert fake.mediaSetSlot("control", "banner", fake.game("control")["media"]["logo"])
    assert media == [("control",)]
    assert fake.game("control")["media"]["banner"].startswith(str(core._root / "data" / "games" / "control" / "media" / "picked"))
    assert fake.mediaUnset("control", "banner") is True and fake.mediaUnset("control", "banner") is False
    assert media == [("control",), ("control",)]


def test_install_job_reports_progress(fake):
    steps, finished = record(fake.progress), record(fake.jobFinished)
    job = fake.install("gog", "1207658930")
    assert job.startswith("job-")
    assert fake.jobs()[0]["id"] == job and fake.jobs()[0]["finished"] is False
    args = until(lambda: finished)[0]
    assert args[0] == job and args[1] is True
    assert steps and steps[-1][1:3] == (50000000000, 50000000000), "progress is in bytes of the install"
    assert next(g for g in fake.sourceLibrary("gog", False) if g["id"] == "1207658930")["installed"] is True


def test_cancel_reaches_the_running_install_only(fake):
    assert fake.cancel("job-none") is False
    steps, finished = record(fake.progress), record(fake.jobFinished)
    job = fake.install("gog", "1207658930")
    until(lambda: steps)
    assert fake.cancel(job) is True
    args = until(lambda: finished)[0]
    assert args[0] == job and args[1] is False and "143" in args[2]
    assert fake.jobs()[0]["cancelled"] is True
    assert fake.cancel(job) is False, "finished"
    row = next(g for g in fake.sourceLibrary("gog", False) if g["id"] == "1207658930")
    assert row["installed"] is False and row["partial_bytes"] > 0


def test_uninstall_via_names_the_store_that_removes_the_files_itself(fake):
    assert fake.uninstallVia("control") == "", "GOG's folder goes to the trash"
    fake.core._game("control")["source"] = "epic"
    assert fake.uninstallVia("control") == "", "Epic Games is off"
    next(s for s in fake.core._data["sources"] if s["id"] == "epic")["enabled"] = True
    assert fake.uninstallVia("control") == "Epic Games"
    assert fake.uninstallVia("no-such-game") == ""


def test_a_failing_job_reports_its_end(fake):
    def broken(source, game_id, progress=None):
        raise RuntimeError("offline")

    fake.core.install = broken
    finished = record(fake.jobFinished)
    job = fake.install("gog", "1")
    assert until(lambda: finished) == [(job, False, "offline")]
    assert fake.jobResult(job) is None, "a failure has no result"


def test_the_client_writes_and_watches_through_the_real_core(app):
    universe_core = pytest.importorskip("universe_core")
    from universe_ui.universe_client import CoreClient

    core = universe_core.Core()
    games = Path(core.data_home()) / "games"
    (games / "sample").mkdir(parents=True, exist_ok=True)
    (games / "sample" / "game.toml").write_text('schema = 1\nid = "sample"\ntitle = "Sample"\n')
    core.reload_game("sample")
    client = CoreClient(core)
    assert [g["id"] for g in client.list()] == ["sample"]
    assert client.currentSession is None

    seen = record(client.error)
    assert client.game("nope") == {} and seen == [("NotFound", "nope")]
    assert client.set("sample", "favorite", "true") and client.game("sample")["favorite"] is True

    written = record(client.entryWritten)
    (games / "sample" / "journal").mkdir()
    until(lambda: written)
    written.clear()
    (games / "sample" / "journal" / "20260911-120000.json").write_text(
        '{"session": "20260911-120000", "game": "sample", "written_at": "2026-09-11T12:10:00+02:00", "lang": "en",'
        ' "title": "First", "provider": "stub", "paragraphs": ["p"], "next_up": "", "images": []}'
    )
    until(lambda: written)
    assert written == [("", "sample")]
    assert [e["title"] for e in client.journal("sample")] == ["First"]
    (games / "sample" / "sessions.jsonl").write_text(
        '{"session": "20260913-120000", "game": "sample", "started_at": "2026-09-13T12:00:00+02:00",'
        ' "ended_at": "2026-09-13T13:00:00+02:00", "duration_s": 3600, "source": "universe", "exit": 0,'
        ' "recording": "' + str(games.parent / "gone.mkv") + '"}\n'
    )
    core.reload_game("sample")
    assert [r["session"] for r in client.recordings("sample")] == ["20260913-120000"]
    assert client.removeRecording("sample", "20260913-120000") is True
    assert client.recordings("sample") == [] and client.game("sample")["stats"]["hours"] == 1.0

    runners = client.runners()
    assert client.setRunnerSetting("dolphin", "batch", "false") and client.runners() != runners
    rom = games.parent / "F-Zero GX.iso"
    rom.write_bytes(b"")
    ident = client.addGame("yuzu", str(rom), "")
    assert ident == "f-zero-gx" and client.game(ident)["effective"]["runner"] == "eden"
    assert client.addGame("dolphin", str(rom), "F-Zero GX") == "" and seen[-1][0] == "Invalid"
    assert client.controllerSetButton("dualsense-edge", "south", "[]") and client.controllerSetButton("dualsense-edge", "south", "null")
    client.shutdown()


def test_a_scan_job_hands_its_games_over_and_their_art_comes_next(fake):
    fake.core.scan = lambda source, progress=None: ["dead-cells", "control"]
    fetched = []
    fake.core.media_refresh_many = lambda ids, force, progress=None: (fetched.append(list(ids)), (0, len(ids)))[1]
    finished = record(fake.jobFinished)
    job = fake.scan("gog")
    until(lambda: finished)
    assert fake.jobResult(job) == ["dead-cells", "control"] and fake.jobs()[0]["result"] == ["dead-cells", "control"]
    until(lambda: fetched == [["dead-cells", "control"]], "the games the scan brought get their art as a media job")
    assert [j["kind"] for j in fake.jobs()] == ["scan", "media"]


def test_an_install_that_lands_a_game_fetches_its_art(fake):
    fake.core.install = lambda source, game_id, progress=None: "dead-cells"
    fetched = []
    fake.core.media_refresh_many = lambda ids, force, progress=None: (fetched.append(list(ids)), (0, len(ids)))[1]
    fake.install("gog", "1207658930")
    until(lambda: fetched == [["dead-cells"]], "the installed game's art comes next")
