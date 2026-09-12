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
    assert core["launch.proton"]["inherited"] is False
    assert core["launch.esync"]["inherited"] is True, "an empty launch key takes the global value"
    assert core["favorite"]["inherited"] is False
    capture = rows_by_key(form, "capture")
    assert capture["enabled"]["value"] is True and capture["enabled"]["type"] == "bool"
    assert "codec" not in capture, "global settings do not belong to a game"
    groups = {g["title"]: g for g in form.groups}
    assert [g["title"] for g in form.groups][:3] == ["Launch", "Desktop and library", "Artwork"]
    assert groups["Launch"]["caps"] is True and groups["Video capture"]["caps"] is False
    assert groups["Video capture"]["meta"] == "v0.1.0 · hooks"
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows)))

    index = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "enabled")
    form.toggle(index)
    assert fake.settings("the-technomancer")["capture"]["enabled"] is False
    assert form.rows[index]["value"] is False


def test_modules_form(api, fake):
    form = api.screens.modules
    form.load()
    rows = form.rows
    tracker = next(g for g in form.groups if g["title"] == "Markdown tracker")
    assert "missing uv" in tracker["warning"] and tracker["off"] is True and tracker["rows"] == []
    enabled = tracker["control"]
    assert rows[enabled]["module"] == "tracker-md" and rows[enabled]["key"] == "enabled"
    assert rows[enabled]["value"] is False
    capture = next(g for g in form.groups if g["title"] == "Video capture")
    assert capture["meta"] == "v0.1.0 · hooks" and capture["warning"] == "" and capture["off"] is False
    assert [rows[i]["key"] for i in capture["rows"]] == ["codec", "fps", "microphone"]
    codec = next(i for i, r in enumerate(rows) if r["module"] == "capture" and r["key"] == "codec")
    assert form.setValue(codec, "av1") is True
    assert fake.getSettings("capture", "")["codec"] == "av1"
    form.loadDoctor()
    assert form.doctor and all("value" in r for r in form.doctor)
    doctor = {g["title"]: g for g in form.doctorGroups}
    assert form.doctorGroups[0]["title"] == "Core" and doctor["Core"]["meta"] == "1 of 2 checks pass"
    assert form.doctor[doctor["GOG"]["rows"][0]]["label"] == "gogdl on PATH"


def test_modules_form_choices(api, fake):
    """Listed choices come with the row; a dynamic setting's arrive from the module, and the
    frame rates above the screen's refresh rate go."""
    form = api.screens.modules
    form._screen_hz = lambda: 90
    form.load()
    rows = rows_by_key(form, "capture")
    assert rows["fps"]["type"] == "int" and rows["fps"]["choices"] == ["auto", "90", "60", "30"]
    journal = rows_by_key(form, "journal")
    assert journal["model"]["dynamic"] is True
    assert journal["model"]["choices"] == ["gpt-6-astra", "gpt-5.6-sol", "gpt-5.5"]
    fps = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "fps")
    assert form.setValue(fps, "auto") is True
    assert fake.getSettings("capture", "")["fps"] == "auto"
    assert form.rows[fps]["display"] == "auto"


def test_sources_browser_statuses(api):
    browser = api.screens.sources
    browser.load()
    assert browser.source == "gog"
    assert not browser.busy
    status = {r["title"]: r["status"] for r in browser.rows}
    assert status["The Technomancer"] == "Installed"
    assert status["Mini Metro"] == "Update available"
    assert status["Stardew Valley"] == "Owned"
    assert [r["title"] for r in browser.updates] == ["Mini Metro"]
    rows = {r["title"]: r for r in browser.rows}
    assert rows["Dead Cells"]["game_id"] == "dead-cells" and rows["Stardew Valley"]["game_id"] == ""
    assert rows["Stardew Valley"]["image"].startswith("https://")
    browser.search("disco")
    assert [r["title"] for r in browser.rows] == ["Disco Elysium"]


def test_sources_browser_keeps_its_fetch(api, fake):
    """A second load reuses what the first fetched; refresh and a finished job fetch again."""
    calls = []
    original = fake.updates
    fake.updates = lambda: calls.append(1) or original()
    browser = api.screens.sources
    browser.load()
    browser.load()
    assert len(calls) == 1
    browser.refresh()
    assert len(calls) == 2


def test_sources_browser_uninstall_and_remove(api, fake):
    browser = api.screens.sources
    browser.load()
    messages = []
    browser.message.connect(messages.append)
    browser.uninstall("dead-cells")
    assert messages == ["Uninstalled Dead Cells"]
    rows = {r["title"]: r for r in browser.rows}
    assert rows["Dead Cells"]["status"] == "Owned" and rows["Dead Cells"]["game_id"] == "dead-cells"
    browser.uninstall("")
    assert len(messages) == 1, "a game outside the library has nothing to uninstall"
    browser.uninstall("dead-cells")
    assert messages[-1] == "Could not uninstall Dead Cells", "the core's refusal reaches the toast"
    browser.remove("the-technomancer")
    assert messages[-1] == "Removed The Technomancer from the library"
    assert fake.game("the-technomancer")["removed"] is True
    browser.remove("no-such-game")
    assert messages[-1] == "Could not remove no-such-game"


def test_path_browser(api, tmp_path):
    (tmp_path / "games" / "Mini Metro").mkdir(parents=True)
    (tmp_path / "games" / "Zeta").mkdir()
    (tmp_path / "games" / ".hidden").mkdir()
    (tmp_path / "games" / "notes.txt").write_text("")
    paths = api.screens.paths
    paths.open(str(tmp_path / "games" / "Mini Metro" / "missing"), False)
    assert paths.path == str(tmp_path / "games" / "Mini Metro"), "a gone path opens at its nearest folder"
    assert paths.up() is True
    assert [e["name"] for e in paths.entries] == ["Mini Metro", "Zeta"]
    paths.open(str(tmp_path / "games"), True)
    assert [(e["name"], e["dir"]) for e in paths.entries] == [("Mini Metro", True), ("Zeta", True), ("notes.txt", False)]
    paths.enter(1)
    assert paths.path == str(tmp_path / "games" / "Zeta") and paths.entries == []
    assert [s["label"] for s in paths.shortcuts][:1] == ["Home"] and paths.shortcuts[-1]["path"] == "/"
    assert paths.display(str(tmp_path)) == str(tmp_path)
    assert paths.display("/") == "/"
    paths.go("/")
    assert paths.atRoot and paths.up() is False


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
