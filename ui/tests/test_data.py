import pytest
from looks import focus_names, notices
from PySide6.QtCore import Qt
from uitest import record, until

from universe_ui.errors import UniverseError


def keys(store):
    return [r["key"] for r in store.rows if r["key"]]


def test_a_games_data_lists_its_saves_its_prefix_and_its_files_and_runs_their_actions(api, fake):
    store = api.screens.gameData
    store.load("the-technomancer")
    until(lambda: store.count > 0 and not store.loading)
    shown = keys(store)
    for key in ("saves_status", "backup", "restore", "export", "move", "winecfg", "winetricks", "run", "kill", "install", "universe", "recordings"):
        assert key in shown, key
    assert "reset" not in shown, "a prefix outside Universe's is not its to reset"
    assert any(k.startswith("restore:backup-") for k in shown), "each backup kept restores on its own"

    done = record(store.finished)
    before = len(fake.core.game_data("the-technomancer")["saves"]["backups"])
    assert store.act("backup") is True
    until(lambda: done)
    assert done[-1][:2] == ("backup", True)
    assert len(fake.core.game_data("the-technomancer")["saves"]["backups"]) == before + 1

    asked = store.question("move")
    assert asked["confirm"] and asked["stale"] == "/mnt/games/gog/the-technomancer", "the confirmation names the path another launcher keeps"
    assert store.act("move") is True
    until(lambda: len(done) == 2)
    assert done[-1][:2] == ("move", True)
    until(lambda: "reset" in keys(store), "moved into Universe's prefixes, the prefix is Universe's to reset")
    assert store.question("reset")["danger"] is True

    assert store.act("winecfg") is True
    until(lambda: len(done) == 3)
    assert fake.core.prefix_tools[-1] == ("the-technomancer", "winecfg", [])
    assert store.runProgram("/mnt/games/setup.exe") is True
    until(lambda: len(done) == 4)
    assert fake.core.prefix_tools[-1] == ("the-technomancer", "run", ["/mnt/games/setup.exe"])
    assert store.act("kill") is True
    until(lambda: len(done) == 5)
    assert done[-1][:2] == ("kill", True)


def test_a_cloud_conflict_on_the_data_page_ends_on_the_side_the_player_keeps(api, fake):
    store = api.screens.gameData
    store.load("the-technomancer")
    until(lambda: "keep_cloud" in keys(store))
    assert "cloud_status" in keys(store) and "keep_local" in keys(store)
    assert store.question("keep_cloud")["danger"] is True and store.question("keep_local")["confirm"]
    done = record(store.finished)
    assert store.act("keep_cloud") is True
    until(lambda: done)
    assert done[-1][:2] == ("keep_cloud", True)
    assert fake.core.cloud_calls == [("the-technomancer", "keep-cloud")]
    until(lambda: "keep_cloud" not in keys(store) and not store.loading, "settled, the page reloads without the choice")
    assert "cloud_status" in keys(store)


def test_a_game_whose_store_syncs_no_saves_shows_no_cloud_row(api, fake):
    store = api.screens.gameData
    store.load("dishonored")
    until(lambda: store.count > 0 and not store.loading)
    assert "cloud_status" not in keys(store)


def test_an_emulator_without_a_title_id_shows_its_save_folder_and_no_backup(api, fake):
    store = api.screens.gameData
    store.load("mini-metro")
    until(lambda: store.count > 0 and not store.loading)
    shown = keys(store)
    assert "saves_folder" in shown and "backup" not in shown and "winecfg" not in shown


def test_the_storage_view_lists_every_root_the_games_and_the_leftovers_it_trashes_on_request(api, fake):
    store = api.screens.storage
    store.loadFree()
    until(lambda: store.free != "", "the free space alone, before any folder is walked")
    store.load()
    until(lambda: store.count > 0 and not store.loading)
    shown = keys(store)
    assert [k for k in shown if k.startswith("root:")] == ["root:" + r for r in ("games", "prefixes", "saves", "recordings", "library", "components", "logs")]
    assert shown.index("game:the-technomancer") < shown.index("game:mini-metro"), "the biggest game first"
    leftover = "trash:/mnt/games/prefixes/cyberpunk-2077-bak"
    assert leftover in shown and store.question(leftover)["danger"] is True
    done = record(store.finished)
    assert store.act(leftover) is True
    until(lambda: done and leftover not in keys(store))
    assert fake.core.trashed == ["/mnt/games/prefixes/cyberpunk-2077-bak"]


def test_every_look_opens_a_games_data_page_and_the_storage_view(api, fake, look):
    look.open("pages/DataPage.qml", look.game("the-technomancer"))
    store = api.screens.gameData
    until(lambda: store.gameId == "the-technomancer" and "backup" in keys(store))
    assert {"cloud_status", "keep_local", "keep_cloud"} <= set(keys(store)), "a conflict's choice in every look"
    fake.core.failing_tools["winetricks"] = "winetricks failed with exit status 127"
    shown = notices(look.engine)
    assert store.act("winetricks") is True
    failed = until(lambda: (n := shown()) and n.get("error") and n)
    assert failed["text"] == fake.core.failing_tools["winetricks"], "the failure and its reason, not a start"
    if look.stacked:
        look.open("pages/StoragePage.qml")
    else:
        look.settings("storage")
    until(lambda: "root:games" in keys(api.screens.storage), "the Storage view walks every root")


@pytest.mark.parametrize("look", ["reprise"], indirect=True)
def test_a_data_page_that_cannot_load_holds_the_focus_on_its_retry(api, fake, look, monkeypatch):
    def unreadable(ident):
        raise UniverseError("Io", "the game's folders could not be read")

    readable = fake.core.game_data
    monkeypatch.setattr(fake.core, "game_data", unreadable)
    look.open("pages/DataPage.qml", look.game("the-technomancer"))
    until(lambda: "retry" in focus_names(look.window), "the failure's one action takes the focus, not the hidden lists")
    monkeypatch.setattr(fake.core, "game_data", readable)
    look.press(Qt.Key.Key_Return)
    until(lambda: "backup" in keys(api.screens.gameData), "A reads the folders again")
    until(lambda: "retry" not in focus_names(look.window) and "dataPage" in focus_names(look.window), "the rows take the focus back")
