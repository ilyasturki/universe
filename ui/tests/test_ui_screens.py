import pytest

from conftest import wait_for


def rows_by_key(form, module=None):
    return {r["key"]: r for r in form.rows if module is None or r["module"] == module}


def test_game_settings_form(api, fake):
    form = api.screens.gameSettings
    form.load("the-technomancer")
    assert form.title == "The Technomancer"
    core = rows_by_key(form, "")
    assert core["launch.proton"]["type"] == "enum"
    assert core["launch.proton"]["choices"] == ["proton-cachyos", "proton-em", "proton-ge"]
    assert core["favorite"]["value"] is True
    capture = rows_by_key(form, "capture")
    assert capture["enabled"]["value"] is True and capture["enabled"]["type"] == "bool"
    assert "codec" not in capture, "global settings do not belong to a game"

    index = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "enabled")
    form.toggle(index)
    assert fake.settings("the-technomancer")["capture"]["enabled"] is False
    assert form.rows[index]["value"] is False


def test_modules_form(api, fake):
    form = api.screens.modules
    form.load()
    rows = form.rows
    tracker = next(r for r in rows if r["type"] == "header" and r["module"] == "tracker-md")
    assert "missing uv" in tracker["detail"]
    enabled = next(i for i, r in enumerate(rows) if r["module"] == "tracker-md" and r["key"] == "enabled")
    assert rows[enabled]["value"] is False
    codec = next(i for i, r in enumerate(rows) if r["module"] == "capture" and r["key"] == "codec")
    assert form.setValue(codec, "av1") is True
    assert fake.getSettings("capture", "")["codec"] == "av1"
    form.loadDoctor()
    assert form.doctor and all("value" in r for r in form.doctor)


def test_sources_browser_statuses(api):
    browser = api.screens.sources
    browser.load()
    assert browser.source == "gog"
    status = {r["title"]: r["status"] for r in browser.rows}
    assert status["The Technomancer"] == "Installed"
    assert status["Mini Metro"] == "Update available"
    assert status["Stardew Valley"] == "Owned"
    assert [r["title"] for r in browser.updates] == ["Mini Metro"]
    browser.search("disco")
    assert [r["title"] for r in browser.rows] == ["Disco Elysium"]


def test_login_flow(api):
    pytest.importorskip("qrcode")
    login = api.screens.login
    login.begin("gog")
    assert login.url.startswith("https://")
    assert login.size >= 21 and all(len(row) == login.size for row in login.matrix)
    login.submit("abc")
    assert login.busy
    args = wait_for(login.finished, 10000)
    assert args is not None and args[0] is True
    assert login.loggedIn()


def test_journal_and_recordings(api):
    journal = api.screens.journal
    journal.load("the-technomancer")
    assert journal.count == 2
    entry = journal.rows[0]
    assert entry["paragraphs"] and entry["dateText"]

    recordings = api.screens.recordings
    recordings.load("the-technomancer")
    assert recordings.count == 2
    row = recordings.rows[0]
    assert row["url"].startswith("file://") and row["durationText"] == "1 h 10" and row["sizeText"] == "2.0 GB"
