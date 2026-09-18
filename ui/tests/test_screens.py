import pytest

from conftest import index_of, rows_by_key, settle, wait_for
from universe_ui.screens.media import _size

PENDING = {"session": "20260912-200000", "game": "the-technomancer", "state": "pending",
           "started_at": "2026-09-12T20:00:00+02:00", "written_at": "", "title": "", "paragraphs": [], "images": []}


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
    assert [g["title"] for g in form.groups][:9] == ["Display", "Overlay", "Advanced", "Proton", "Sync", "Upscaling", "Launch", "Desktop and library", "Artwork"], \
        "the launch page's cards, the runner's, then the program"
    assert groups["Launch"]["caps"] is True and groups["Video capture"]["caps"] is False
    assert groups["Display"]["meta"] == "DP-1 2560×1440 @ 144 Hz" and groups["Upscaling"]["meta"] == "AMD Radeon RX 7900 GRE · RDNA 3"
    assert groups["Video capture"]["meta"] == "v0.1.0"
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows)))

    index = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "enabled")
    form.toggle(index)
    assert fake.settings("the-technomancer")["capture"]["enabled"] is False
    assert form.rows[index]["value"] is False


def test_modules_list(api, fake):
    journal_module = next(m for m in fake.core._data["modules"] if m["id"] == "journal")
    journal_module.update(enabled=False, available=False, missing=["ffmpeg"])
    form = api.screens.modules
    form.load()
    assert [r["module"] for r in form.rows] == ["capture", "journal"], "the manifests' order, sources apart"
    assert all(r["type"] == "action" and r["key"] == "module" and r["switch"] is True and r["source"] is False for r in form.rows)
    assert [(g["title"], [form.rows[i]["module"] for i in g["rows"]], g["off"]) for g in form.groups] == [("", ["capture"], False), ("Off", ["journal"], True)]
    capture = form.rows[form.indexOf("capture")]
    assert capture["label"] == "Video capture" and capture["value"] is True and capture["display"] == "On" and capture["meta"] == "v0.1.0"
    journal = form.rows[form.indexOf("journal")]
    assert journal["value"] is False and journal["display"] == "Unavailable" and journal["detail"] == "Cannot be enabled: missing ffmpeg"
    form.toggle(form.indexOf("capture"))
    assert form.rows[form.indexOf("capture")]["value"] is False and form.rows[form.indexOf("capture")]["display"] == "Off"
    assert next(m for m in fake.modules() if m["id"] == "capture")["enabled"] is False
    form.loadDoctor()
    wait_for(form.doctorChanged, 3000)  # the checks run off the UI thread
    assert form.doctor and all("value" in r for r in form.doctor)
    doctor = {g["title"]: g for g in form.doctorGroups}
    assert form.doctorGroups[0]["title"] == "Core" and doctor["Core"]["meta"] == "1 of 2 checks pass"
    assert form.doctor[doctor["GOG"]["rows"][0]]["label"] == "gogdl on PATH", "a source's checks are grouped under its name"


def test_sources_list(api, fake):
    form = api.screens.sourceList
    form.load()
    wait_for(form.rowsChanged, 3000)  # the listing probes the logins: off the UI thread
    assert [r["module"] for r in form.rows] == ["gog"]
    gog = form.rows[0]
    assert gog["section"] == "Sources" and gog["switch"] is True and gog["source"] is True
    assert gog["label"] == "GOG" and gog["value"] is True and gog["display"] == "On" and gog["meta"] == "v0.1.0"
    assert [(g["title"], g["rows"]) for g in form.groups] == [("", [0])]
    form.toggle(0)
    wait_for(form.rowsChanged, 3000)
    assert next(s for s in fake.sources() if s["id"] == "gog")["enabled"] is False
    assert form.rows[0]["value"] is False and form.rows[0]["display"] == "Off"
    assert [g["title"] for g in form.groups] == ["Off"], "no empty card for the running ones"


def test_source_form(api, fake):
    form = api.screens.source
    form.load("gog")
    wait_for(form.rowsChanged, 3000)
    assert form.info["name"] == "GOG" and form.info["source"] is True and form.info["logged_in"] is True and form.info["user"] == "yasso"
    rows = form.rows
    assert rows[0]["key"] == "enabled" and rows[0]["value"] is True and rows[0]["disabled"] is False
    groups = {g["title"]: g for g in form.groups}
    assert [rows[i]["key"] for i in groups["Sign-in"]["rows"]] == ["logged_in", "link", "code"]
    assert rows[groups["Sign-in"]["rows"][0]]["type"] == "info" and rows[groups["Sign-in"]["rows"][0]]["detail"] == "yasso"
    assert [rows[i]["key"] for i in groups["Settings"]["rows"]] == ["games_dir", "scan_dirs", "platform", "with_dlcs", "auth_path", "install_timeout_s"], "every setting: a source's are all global"
    platform = index_of(form, "platform")
    assert rows[platform]["choices"] == ["windows", "linux"]
    assert form.setValue(platform, "linux") is True
    wait_for(form.rowsChanged, 3000)
    assert fake.getSourceSettings("gog")["platform"] == "linux"
    assert form.rows[index_of(form, "platform")]["value"] == "linux"
    form.toggle(0)
    wait_for(form.rowsChanged, 3000)
    assert form.info["enabled"] is False and [r["key"] for r in form.rows] == ["enabled"], "off: the switch alone"
    gog = next(s for s in fake.core._data["sources"] if s["id"] == "gog")
    gog.update(available=False, missing=["gogdl"])
    form.load("gog")
    wait_for(form.rowsChanged, 3000)
    assert "missing gogdl" in form.info["warning"] and form.rows[0]["disabled"] is True
    form.load("capture")
    wait_for(form.rowsChanged, 3000)
    assert form.rows == [] and form.info == {}, "a module is not a source"


def test_module_form(api, fake):
    journal_module = next(m for m in fake.core._data["modules"] if m["id"] == "journal")
    journal_module.update(enabled=False, available=False, missing=["ffmpeg"])
    form = api.screens.module
    form.load("journal")
    assert form.info["name"] == "Play journal" and "missing ffmpeg" in form.info["warning"] and form.info["enabled"] is False
    assert [r["key"] for r in form.rows] == ["enabled"], "off: the switch alone"
    assert form.rows[0]["value"] is False and form.rows[0]["disabled"] is True
    assert form.groups == [{"title": "", "meta": "", "warning": "", "caps": False, "control": -1, "off": False, "rows": [0]}], "the page header carries the name and the warning"
    form.load("capture")
    assert form.info["meta"] == "v0.1.0" and form.info["warning"] == "" and form.info["source"] is False
    rows = form.rows
    assert rows[0]["key"] == "enabled" and rows[0]["value"] is True and rows[0]["disabled"] is False
    settings = next(g for g in form.groups if g["title"] == "Settings")
    assert [rows[i]["key"] for i in settings["rows"]] == ["codec", "quality", "fps", "size", "container", "audio", "audio_codec", "audio_bitrate", "min_duration_s", "window_wait_s"]
    assert form.setValue(index_of(form, "codec"), "av1") is True
    assert fake.getSettings("capture", "")["codec"] == "av1"
    form.toggle(0)
    assert form.info["enabled"] is False and [r["key"] for r in form.rows] == ["enabled"]
    form.load("nope")
    assert form.rows == [] and form.info == {}


def test_module_form_choices(api, fake):
    form = api.screens.module
    form.load("capture")
    rows = rows_by_key(form, "capture")
    assert rows["fps"]["type"] == "int" and rows["fps"]["choices"] == ["auto", "120", "90", "60", "30"]
    form.load("journal")
    wait_for(form.rowsChanged, 3000)  # the dynamic choices come back from a thread
    journal = rows_by_key(form, "journal")
    assert journal["model"]["choices"] == ["gpt-6-astra", "gpt-5.6-sol", "gpt-5.5"]
    form.load("capture")
    fps = index_of(form, "fps")
    assert form.setValue(fps, "auto") is True
    assert fake.getSettings("capture", "")["fps"] == "auto"
    assert form.rows[fps]["display"] == "auto"


def test_launch_form(api, fake):
    form = api.screens.launch
    form.load()
    assert form.screen == "DP-1 2560×1440 @ 144 Hz"
    keys = fake.launchKeys("global", fake.screenMode("DP-1"))
    assert {k["scope"] for k in keys} == {"both"} and "prefix" not in [k["key"] for k in keys]
    expected = [(section, ["launch." + k["key"] for k in keys if k["section"] == section and not k["runners"]]) for section in ("Display", "Overlay", "Advanced")]
    expected[1][1].append("desktop.hide_cursor")
    assert [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups] == expected, "beginner first; a runner's keys sit on its page"
    assert expected[0][1] == ["launch.gamescope", "launch.gamescope_resolution", "launch.gamescope_refresh", "launch.gamescope_adaptive_sync"]
    assert expected[1][1] == ["launch.mangohud", "launch.fps_limit", "launch.pause_on_home", "desktop.hide_cursor"]
    assert expected[2][1] == ["launch.gamescope_scaler", "launch.gamescope_filter", "launch.gamescope_sharpness", "launch.gamescope_args"]
    assert form.groups[0]["meta"] == form.screen and form.groups[1]["meta"] == ""
    rows = rows_by_key(form)
    assert rows["launch.gamescope"]["detail"] == next(k["description"] for k in keys if k["key"] == "gamescope")
    assert rows["launch.gamescope"]["value"] is True
    assert rows["launch.gamescope_resolution"]["type"] == "string" and rows["launch.gamescope_resolution"]["value"] == "auto"
    assert rows["launch.gamescope_resolution"]["choices"] == ["auto", "2560x1440", "1920x1080", "1280x720"], "the screen, then the standard heights at its aspect"
    assert rows["launch.gamescope_refresh"]["choices"] == ["auto", "144", "120", "100", "90", "75", "60", "50", "48", "40", "30"]
    assert rows["launch.gamescope_scaler"]["type"] == "enum" and rows["launch.gamescope_scaler"]["value"] == "default"
    assert rows["launch.gamescope_scaler"]["choices"] == ["default", "auto", "integer", "fit", "fill", "stretch"]
    assert rows["launch.gamescope_scaler"]["choiceValues"] == ["", "auto", "integer", "fit", "fill", "stretch"]
    assert rows["launch.gamescope_sharpness"]["value"] == "default"
    assert rows["launch.fps_limit"]["value"] == "auto" and rows["launch.fps_limit"]["display"] == "auto · 144", "auto shows the rate it stands for"
    assert rows["launch.fps_limit"]["choices"] == ["auto", "none", "144", "120", "100", "90", "75", "60", "50", "48", "40", "30"]
    assert rows["launch.gamescope_adaptive_sync"]["value"] is False and rows["launch.gamescope_args"]["value"] == ""
    assert rows["desktop.hide_cursor"]["value"] is True and "launch.esync" not in rows

    index = index_of(form, "launch.gamescope_scaler")
    assert form.setValue(index, "integer") is True
    assert fake.config()["launch"]["gamescope_scaler"] == "integer"
    assert form.setValue(index, "default") is True
    assert "gamescope_scaler" not in fake.config()["launch"], "the sentinel clears the key"
    assert form.setValue(index_of(form, "launch.gamescope_sharpness"), "7") is True, "a typed value passes through"
    assert fake.config()["launch"]["gamescope_sharpness"] == 7
    form.load()
    assert rows_by_key(form)["launch.gamescope_sharpness"]["value"] == "7"
    form.toggle(index_of(form, "launch.gamescope"))
    assert fake.config()["launch"]["gamescope"] is False
    index = index_of(form, "launch.fps_limit")
    assert form.setValue(index, "none") is True
    assert fake.config()["launch"]["fps_limit"] == "none"
    assert rows_by_key(form)["launch.fps_limit"]["display"] == "none"
    assert form.setValue(index, "auto") is True
    form.toggle(index_of(form, "launch.gamescope"))
    assert form.setValue(index_of(form, "launch.gamescope_refresh"), "30") is True
    assert rows_by_key(form)["launch.fps_limit"]["display"] == "auto · 30", "auto follows the gamescope rate the game sees"
    form.toggle(index_of(form, "launch.gamescope"))
    assert rows_by_key(form)["launch.fps_limit"]["display"] == "auto · 144", "on the desktop the gamescope rate means nothing"


def test_game_settings_mirrors_the_cards(api, fake):
    form = api.screens.gameSettings
    form.load("the-technomancer")
    catalogue = fake.launchKeys("game", fake.screenMode("DP-1"))
    cards = {g["title"]: [form.rows[i]["key"] for i in g["rows"]] for g in form.groups}
    for section in ("Display", "Overlay", "Advanced", "Sync", "Upscaling"):
        assert cards[section] == ["launch." + k["key"] for k in catalogue if k["section"] == section], section
    assert cards["Proton"] == ["launch.proton", "launch.wayland", "launch.hdr", "launch.prefix"], "the runner's card carries its name"
    assert cards["Launch"] == ["launch.runner", "launch.exe", "launch.wrapper", "launch.args", "launch.working_dir"]
    rows = rows_by_key(form, "")
    assert rows["launch.fps_limit"]["value"] == "auto" and rows["launch.fps_limit"]["inherited"] is True and rows["launch.fps_limit"]["display"] == "auto · 144"
    assert rows["launch.esync"]["detail"].startswith("Faster thread synchronisation")
    assert rows["launch.dlss_upgrade"]["detail"].endswith("Not for your GPU.") and rows["launch.fsr4_upgrade"]["detail"].endswith("Works on your GPU.")
    assert rows["launch.gamescope"]["detail"].startswith("Run the game in a window"), "no GPU note outside Upscaling"
    assert rows["launch.gamescope_resolution"]["value"] == "auto" and rows["launch.gamescope_resolution"]["inherited"] is True
    assert rows["launch.gamescope_resolution"]["choices"][:2] == ["auto", "2560x1440"]
    assert rows["launch.gamescope_scaler"]["value"] == "default" and rows["launch.gamescope_scaler"]["inherited"] is True
    assert rows["launch.gamescope_adaptive_sync"]["value"] is False and rows["launch.gamescope_adaptive_sync"]["inherited"] is True
    assert form.setValue(index_of(form, "launch.gamescope_resolution"), "1920x1080") is True
    assert fake.game("the-technomancer")["launch"]["gamescope_resolution"] == "1920x1080"
    rows = rows_by_key(form, "")
    assert rows["launch.gamescope_resolution"]["value"] == "1920x1080" and rows["launch.gamescope_resolution"]["inherited"] is False
    form.load("mini-metro")
    titles = [g["title"] for g in form.groups]
    assert "Proton" not in titles and "Sync" not in titles and "Upscaling" not in titles and "Eden" not in titles, "an emulator has no runner card"
    assert "launch.wrapper" in [r["key"] for r in form.rows if r["section"] == "Launch"]


def test_sources_browser_statuses(api):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    assert browser.source == "gog"
    status = {r["title"]: r["status"] for r in browser.rows}
    assert status["The Technomancer"] == "Installed"
    assert status["Mini Metro"] == "Update available"
    assert status["Stardew Valley"] == "Owned"
    assert status["Disco Elysium"] == f"Paused · {_size(6100000000)} of {_size(15400000000)} kept"
    assert [r["title"] for r in browser.updates] == ["Mini Metro"]
    rows = {r["title"]: r for r in browser.rows}
    assert rows["Dead Cells"]["game_id"] == "dead-cells" and rows["Stardew Valley"]["game_id"] == ""
    assert rows["Stardew Valley"]["image"].startswith("https://")
    assert (rows["The Technomancer"]["sizeText"], rows["The Technomancer"]["sizeKind"]) == (_size(8100000000), "disk")
    assert (rows["Stardew Valley"]["sizeText"], rows["Stardew Valley"]["sizeKind"]) == ("", ""), "unknown until peeked"
    assert (rows["Disco Elysium"]["sizeText"], rows["Disco Elysium"]["action"], rows["Disco Elysium"]["partial"]) == (_size(15400000000), "Resume", True)
    assert browser.libraryAt == "2026-09-11T19:03:00+02:00" and (browser.libraryAge.endswith("Sep") or browser.libraryAge.endswith("ago"))


def test_sources_browser_refresh_hits_the_store_and_page_open_does_not(api, fake):
    core = fake._core
    browser = api.screens.sources
    browser.load()
    settle(browser)
    assert core.library_calls == [False], "opening the page serves the cache"
    assert "Alan Wake" not in [r["title"] for r in browser.rows]
    browser.refresh()
    settle(browser)
    assert core.library_calls == [False, True], "Y asks the store"
    assert "Alan Wake" in [r["title"] for r in browser.rows], "a game bought since shows up"
    assert browser.libraryAge == "just now"
    browser.uninstall("dead-cells")
    settle(browser)
    assert core.library_calls == [False, True, False], "a reload after a job stays off the network"


def test_sources_browser_free_space_and_a_failed_refresh(api, fake, tmp_path):
    import shutil

    from universe_ui.universe_client import UniverseError

    core = fake._core
    core._data["sources"][0]["games_dir"] = str(tmp_path)
    browser = api.screens.sources
    browser.load()
    settle(browser)
    assert browser.error == "" and browser.freeSpace > 0
    assert abs(browser.freeSpace - shutil.disk_usage(tmp_path).free) < 1 << 30, "the install folder's free bytes"
    before = [r["title"] for r in browser.rows]
    library = core.library

    def offline(source, refresh):
        raise UniverseError("Io", "gog library failed: offline")

    core.library = offline
    browser.refresh()
    settle(browser)
    assert browser.error == "gog library failed: offline"
    assert [r["title"] for r in browser.rows] == before, "the listing shown stays"
    core.library = library
    browser.refresh()
    settle(browser)
    assert browser.error == ""


def test_sources_browser_peek_fills_a_size_once(api, fake):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    rows = {r["title"]: i for i, r in enumerate(browser.rows)}
    browser.peek(rows["The Technomancer"])
    assert fake._core._source_game("gog", "1972906591").get("download_size") is None, "installed rows have their size"
    seen = []
    browser.rowsChanged.connect(lambda: seen.append(1))
    browser.peek(rows["Stardew Valley"])
    wait_for(browser.rowsChanged, 5000)
    row = browser.rows[rows["Stardew Valley"]]
    assert (row["sizeText"], row["sizeKind"], row["disk_size"]) == (_size(500000000), "download", 1100000000)
    assert not browser.busy, "a peek never shows Loading…"


def test_sources_browser_cancel_pauses_the_install(api, fake):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    messages = []
    browser.message.connect(messages.append)
    index = next(i for i, r in enumerate(browser.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    job = browser.install(index)
    assert job and browser.job["game"] == "1207658930"
    rebuilds = []
    browser.rowsChanged.connect(lambda: rebuilds.append(1))
    wait_for(browser.jobChanged, 5000)
    wait_for(browser.jobChanged, 5000)
    row = browser.rows[index]
    assert row["busy"] and row["action"] == "Cancel" and row["status"] == "Installing…"
    assert browser.job["message"].startswith("Installing The Witcher 3: Wild Hunt · ") and f" of {_size(50000000000)}" in browser.job["message"]
    assert rebuilds == [], "progress moves the job line, not the rows"
    assert browser.install(index) == "", "one job at a time"
    assert browser.cancel() is True
    assert browser.job["cancelled"] and browser.job["message"].startswith("Stopping")
    assert wait_for(browser.message, 5000)
    settle(browser)
    assert messages[-1].startswith("Stopped installing The Witcher 3: Wild Hunt · ") and messages[-1].endswith("kept, resume any time")
    row = browser.rows[index]
    assert row["partial"] and row["action"] == "Resume" and row["status"].startswith("Paused · ")
    assert browser.cancel() is False, "nothing running"
    browser.install(index)
    assert wait_for(browser.message, 10000)[0] == "Installing 1207658930: done"
    settle(browser)
    row = next(r for r in browser.rows if r["title"] == "The Witcher 3: Wild Hunt")
    assert row["installed"] and row["sizeText"] == _size(50000000000) and row["status"] == "Installed"
    browser.search("disco")
    settle(browser)
    assert [r["title"] for r in browser.rows] == ["Disco Elysium"]


def test_sources_browser_keeps_its_fetch(api, fake):
    calls = []
    original = fake.updates
    fake.updates = lambda: calls.append(1) or original()
    browser = api.screens.sources
    browser.load()
    settle(browser)
    browser.load()
    settle(browser)
    assert len(calls) == 1
    browser.refresh()
    settle(browser)
    assert len(calls) == 2


def test_sources_browser_uninstall_and_remove(api, fake):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    messages = []
    browser.message.connect(messages.append)
    browser.uninstall("dead-cells")
    settle(browser)
    assert messages == ["Uninstalled Dead Cells"]
    rows = {r["title"]: r for r in browser.rows}
    assert rows["Dead Cells"]["status"] == "Owned" and rows["Dead Cells"]["game_id"] == "dead-cells"
    browser.uninstall("")
    settle(browser)
    assert len(messages) == 1, "a game outside the library has nothing to uninstall"
    browser.uninstall("dead-cells")
    settle(browser)
    assert messages[-1] == "Could not uninstall Dead Cells", "the core's refusal reaches the toast"
    browser.remove("the-technomancer")
    settle(browser)
    assert messages[-1] == "Removed The Technomancer from the library"
    assert fake.game("the-technomancer")["removed"] is True
    browser.remove("no-such-game")
    settle(browser)
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
    assert paths.display("/mnt/games") == "/mnt/games", "outside home, verbatim (tmp_path sits under HOME in the nix sandbox)"
    assert paths.display("/") == "/"
    paths.go("/")
    assert paths.atRoot and paths.up() is False


def test_login_flow(api):
    login = api.screens.login
    login.begin("gog")
    assert login.url.startswith("https://")
    assert login.size >= 21 and all(len(row) == login.size for row in login.matrix)
    login.submit("abc")
    assert login.busy
    args = wait_for(login.finished, 10000)
    assert args is not None and args[0] is True


def test_removing_a_recording_or_an_entry_reloads_both_lists(api, fake):
    journal, recordings = api.screens.journal, api.screens.recordings
    journal.load("the-technomancer")
    recordings.load("the-technomancer")
    assert journal.count == 2 and recordings.count == 2
    entry, row = journal.rows[0], recordings.rows[0]
    assert entry["paragraphs"] and entry["dateText"] and entry["hasRecording"] is True
    assert row["url"].startswith("file://") and row["sizeText"] == "2.0 GB" and row["hasJournal"] is True
    assert row["durationText"] == "1 h 10" and row["gameTitle"] == "The Technomancer"
    assert entry["images"] and all(i.startswith("file://") for i in entry["images"]), "the core hands the images out absolute"
    session = row["session"]

    assert recordings.remove("the-technomancer", session) is True
    assert recordings.count == 1 and session not in recordings.frameMap
    journal.load("the-technomancer")
    assert [r["hasRecording"] for r in journal.rows if r["session"] == session] == [False]

    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))
    assert recordings.remove("the-technomancer", session) is False and errors == ["NotFound"]

    assert journal.remove("the-technomancer", session) is True
    assert journal.count == 1 and all(r["session"] != session for r in journal.rows)


def test_journal_rows_carry_state_and_duration(api, fake):
    entries = fake.core._data["journal"]["the-technomancer"]
    entries.insert(0, dict(PENDING))
    entries.append({"session": "20260905-190000", "game": "the-technomancer", "state": "failed", "duration_s": 2520,
                    "started_at": "2026-09-05T19:00:00+02:00", "written_at": "2026-09-05T19:50:00+02:00", "title": "",
                    "paragraphs": ["codex timed out after 30 min"], "images": []})
    journal = api.screens.journal
    journal.load("the-technomancer")
    rows = journal.rows
    assert [r["state"] for r in rows] == ["pending", "written", "written", "failed"]
    pending, first, second, failed = rows
    assert pending["title"] == "" and pending["durationText"] == "" and pending["reason"] == ""
    assert pending["started_at"] == "2026-09-12T20:00:00+02:00" and "20:00" in pending["dateText"]
    assert first["durationText"] == "1 h 10" and first["duration_s"] == 4215 and second["durationText"] == "1 h 17"
    assert failed["title"] == "Journal failed" and failed["reason"] == "codex timed out after 30 min"
    assert failed["durationText"] == "42 min" and failed["blocks"] == ["codex timed out after 30 min"]


def test_pending_journals_announce_each_session_once(api, fake):
    pending = api.screens.pendingJournals
    assert pending.count == 0
    seen = []
    pending.appeared.connect(lambda session, title: seen.append(("appeared", session, title)))
    pending.resolved.connect(lambda session, game, state, text: seen.append(("resolved", session, game, state, text)))
    entries = fake.core._data["journal"]["the-technomancer"]
    entries.insert(0, dict(PENDING))
    fake.entryWritten.emit("20260912-200000", "the-technomancer")
    assert pending.count == 1 and pending.rows[0]["title"] == "The Technomancer"
    fake.entryWritten.emit("", "the-technomancer")
    fake.sessionEnded.emit("20260912-200000", "the-technomancer", 60)
    assert seen == [("appeared", "20260912-200000", "The Technomancer")]

    entries[0].update(state="written", title="Back to Noctis", paragraphs=["p"], written_at="2026-09-12T20:50:00+02:00")
    fake.entryWritten.emit("20260912-200000", "the-technomancer")
    assert pending.count == 0
    assert seen[-1] == ("resolved", "20260912-200000", "the-technomancer", "written", "Back to Noctis")

    entries.insert(0, {"session": "20260913-100000", "game": "the-technomancer", "state": "pending",
                       "started_at": "2026-09-13T10:00:00+02:00", "title": "", "paragraphs": [], "images": []})
    fake.sessionEnded.emit("20260913-100000", "the-technomancer", 60)
    entries[0].update(state="failed", paragraphs=["codex timed out"])
    fake.entryWritten.emit("20260913-100000", "the-technomancer")
    assert seen[-2:] == [("appeared", "20260913-100000", "The Technomancer"),
                         ("resolved", "20260913-100000", "the-technomancer", "failed", "codex timed out")]


def test_album_and_news_span_every_game(api):
    album = api.screens.album
    album.loadAll()
    assert album.count == 2 and album.gameId == ""
    assert album.rows[0]["gameTitle"] == "The Technomancer" and album.rows[0]["gameId"] == "the-technomancer"
    assert album.rows[0]["created_at"] >= album.rows[1]["created_at"], "newest first"
    news = api.screens.news
    news.loadAll()
    assert news.count == 2 and news.rows[0]["gameTitle"] == "The Technomancer" and news.gameId == ""
    assert news.rows[0]["written_at"] >= news.rows[1]["written_at"]


def test_screenshots_list_per_game_and_across_games(api, fake):
    shots = api.screens.shots
    shots.load("the-technomancer")
    assert shots.count > 0 and shots.gameId == "the-technomancer"
    first = shots.rows[0]
    assert first["gameId"] == "the-technomancer" and first["name"].endswith(".png") and first["url"].startswith("file://")
    assert first["session"] != "" and first["hasJournal"] is True, "a shot taken during a journaled session knows its entry"
    assert [r["name"] for r in shots.rows] == sorted((r["name"] for r in shots.rows), reverse=True), "newest first"
    per_game = shots.count
    shots.loadAll()
    assert shots.gameId == "" and shots.count >= per_game and first in shots.rows
    name, ident, before = shots.rows[0]["name"], shots.rows[0]["gameId"], shots.count
    assert shots.remove(ident, name)
    assert shots.count == before - 1 and not any(r["name"] == name and r["gameId"] == ident for r in shots.rows)


def test_media_timeline_merges_the_three_kinds(api):
    media = api.screens.media
    media.load()
    kinds = {r["kind"] for r in media.rows}
    assert kinds == {"shot", "recording", "journal"}
    whens = [r["when"] for r in media.rows]
    assert whens == sorted(whens, reverse=True), "one timeline, newest first"
    shot = next(r for r in media.rows if r["kind"] == "shot")
    assert shot["image"].startswith("file://") and shot["name"].endswith(".png") and shot["gameTitle"]
    entry = next(r for r in media.rows if r["kind"] == "journal")
    assert entry["title"] and entry["hasJournal"] and entry["session"]
    rec = next(r for r in media.rows if r["kind"] == "recording")
    assert rec["title"] and rec["session"] and rec["path"]


def test_journal_paragraphs_become_markdown_blocks():
    from universe_ui.screens.media import markdown_blocks

    assert markdown_blocks(["Intro.", "- **A:** one", "- **B:** two", "Outro.", "1. first", "2. second"]) == [
        "Intro.", "- **A:** one\n- **B:** two", "Outro.", "1. first\n2. second"]
    assert markdown_blocks([]) == []


def test_recording_frames_are_sampled_from_the_file(api):
    import shutil

    from universe_ui.screens import media

    if not (shutil.which("ffmpeg") and shutil.which("ffprobe")):
        pytest.skip("ffmpeg and ffprobe sample the frames")
    recordings = api.screens.recordings
    recordings.load("the-technomancer")
    session = recordings.rows[0]["session"]
    recordings.select(session)
    for _ in range(80):
        if recordings.frameMap[session]["complete"]:
            break
        assert wait_for(recordings.framesChanged, 10000) is not None
    frames = recordings.frameMap[session]
    assert frames["complete"] and all(f.startswith("file://") for f in frames["frames"])
    # The session lasted 1 h 10; the row carries the clip's 20 s, and the seeks follow the file.
    assert 19.5 < frames["duration"] < 20.5
    assert frames["thumbnail"] == frames["frames"][media.THUMB]

    again = media.RecordingsList(api.universe)
    again.load("the-technomancer")
    assert again.frameMap[session]["complete"]
    again.shutdown()
