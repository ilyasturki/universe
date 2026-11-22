import threading

import pytest
from looks import FocusTrace, activate, content_rows, focus_names, held_notices, invoke, notices, read, settle
from PySide6.QtCore import SIGNAL, QObject, Qt
from uitest import descendant, record, until

from universe_ui.errors import UniverseError


def keys(store):
    return [r["key"] for r in store.rows if r["key"]]


def test_backups_behind_a_damaged_list_can_still_be_exported(api, fake, monkeypatch):
    readable = fake.core.game_data

    def damaged(ident):
        data = readable(ident)
        data["saves"].update(backups=[], unreadable="/saves/mapping.yaml", error="/saves/mapping.yaml: damaged")
        return data

    monkeypatch.setattr(fake.core, "game_data", damaged)
    store = api.screens.gameData
    store.load("the-technomancer")
    until(lambda: store.count > 0 and not store.loading)
    rows = {r["key"]: r for r in store.rows if r["key"]}
    assert "/saves/mapping.yaml" in rows["saves_status"]["detail"]
    assert rows["restore"]["disabled"] and not rows["export"]["disabled"]


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


def test_a_data_action_reports_once_whether_its_page_stays_or_closes(api, fake, look, monkeypatch):
    store = api.screens.gameData
    gate, real, failing = threading.Event(), fake.core.saves_backup, []

    def held(ident):
        gate.wait(10)
        if failing:
            raise UniverseError("Io", "the saves folder could not be read")
        return real(ident)

    monkeypatch.setattr(fake.core, "saves_backup", held)
    posted, clear = held_notices(look.engine)

    def back_up(close):
        gate.clear()
        clear()
        page = look.open("pages/DataPage.qml", look.game("the-technomancer"))
        until(lambda: "backup" in keys(store))
        activate(page, store.rows, key="backup")
        until(lambda: store.busy == "backup")
        while close and store.gameId:
            look.press(Qt.Key.Key_Escape)
        if not close:
            assert store.receivers(SIGNAL("finished(QString,bool,QString)")) == 1, "Notices folds a repeated text, so one listener is the guard"
        gate.set()
        until(lambda: posted() and not store.busy, "the result reaches the notices")
        return [(n["error"], n["text"]) for n in posted()]

    failing.append(True)
    assert back_up(close=True) == [(True, "the saves folder could not be read")], "the failure outlives its page"
    failing.clear()
    assert [error for error, _ in back_up(close=True)] == [False], "so does the success"
    assert [error for error, _ in back_up(close=False)] == [False], "one notice with the page still open"


def test_a_leftover_trash_reports_once_whether_its_storage_page_stays_or_closes(api, fake, look, monkeypatch):
    store = api.screens.storage
    gate, real, failing, started = threading.Event(), fake.core.trash_leftover, [], []

    def held(path):
        started.append(path)
        gate.wait(10)
        if failing:
            raise UniverseError("Io", "the trash is full")
        return real(path)

    monkeypatch.setattr(fake.core, "trash_leftover", held)
    posted, clear = held_notices(look.engine)
    shown = "storagePage" if look.stacked else "settingsCards"

    def trash(close):
        gate.clear()
        clear()
        page = look.open("pages/StoragePage.qml") if look.stacked else look.settings("storage")
        until(lambda: any(k.startswith("trash:") for k in keys(store)) and not store.loading, "the leftovers are measured")
        rows = store.rows if look.stacked else content_rows(page)
        key = next(r["key"] for r in rows if r["key"].startswith("trash:"))
        activate(page, rows, key=key)
        dialog = look.dialog() if look.stacked else page.findChild(QObject, "dialog")
        until(lambda: dialog.property("open") is True, "the trash asks first")
        look.press(Qt.Key.Key_Right if look.stacked else Qt.Key.Key_Down)
        until(lambda: dialog.property("index") == 1, "the cursor on Move to Trash")
        look.press(Qt.Key.Key_Return)
        until(lambda: dialog.property("open") is False)
        until(lambda: started, "Trash starts it")
        started.clear()
        while close and shown in focus_names(look.window):
            look.press(Qt.Key.Key_Escape)
        if not close:
            assert store.receivers(SIGNAL("finished(QString,bool,QString)")) == 1, "Notices folds a repeated text, so one listener is the guard"
        gate.set()
        until(posted, "the result reaches the notices")
        until(lambda: key.partition(":")[2] in fake.core.trashed or failing)
        return [(n["error"], n["text"]) for n in posted()]

    failing.append(True)
    assert trash(close=True) == [(True, "the trash is full")], "the failure outlives its page"
    failing.clear()
    assert [error for error, _ in trash(close=True)] == [False], "so does the success"
    assert [error for error, _ in trash(close=False)] == [False], "one notice with the page still open"


def listed(look, page):
    """The rows a Data or Storage view shows: Reprise's Settings cards, the others' list with its status rows."""
    return content_rows(page) if not look.stacked else read(descendant(page, "settingsRows"), "model")


def under_cursor(page):
    """The row under a console list's cursor while the list holds the focus."""
    rows = descendant(page, "settingsRows")
    return read(rows, "currentRow") or {} if rows.property("activeFocus") else {}


def test_a_data_page_that_cannot_load_holds_the_focus_on_its_retry(api, fake, look, monkeypatch):
    gate, readable, failing = threading.Event(), fake.core.game_data, [True]

    def held(ident):
        gate.wait(10)
        if failing:
            raise UniverseError("Io", "the game's folders could not be read")
        return readable(ident)

    monkeypatch.setattr(fake.core, "game_data", held)
    page = look.open("pages/DataPage.qml", look.game("the-technomancer"))
    if look.stacked:
        until(lambda: [r["key"] for r in listed(look, page)] == ["reading"], "a read under way says so in the list")
        assert not descendant(page, "emptyLabel").isVisible(), "and only there"
    gate.set()
    if look.stacked:
        until(lambda: under_cursor(page).get("key") == "retry", "the failure's one action takes the focus")
        assert under_cursor(page)["detail"] == "the game's folders could not be read"
        assert not descendant(page, "emptyLabel").isVisible()
    else:
        until(lambda: "retry" in focus_names(look.window), "the failure's one action takes the focus, not the hidden lists")
    failing.clear()
    look.press(Qt.Key.Key_Return)
    until(lambda: "backup" in keys(api.screens.gameData), "A reads the folders again")
    if look.stacked:
        until(lambda: "retry" not in [r["key"] for r in listed(look, page)] and under_cursor(page).get("key"), "the rows take the cursor")
    else:
        until(lambda: "retry" not in focus_names(look.window) and "dataPage" in focus_names(look.window), "the rows take the focus back")


def test_storage_says_it_is_measuring_and_offers_a_retry_when_the_measure_fails(api, fake, look, monkeypatch):
    gate, real, failing = threading.Event(), fake.core.storage, [True]

    def held():
        gate.wait(10)
        if failing:
            raise UniverseError("Io", "the games folder could not be read")
        return real()

    monkeypatch.setattr(fake.core, "storage", held)
    page = look.open("pages/StoragePage.qml") if look.stacked else look.settings("storage")
    cards = "settingsRows" if look.stacked else "settingsCards"

    def shown():
        return [r["key"] for r in listed(look, page)]

    until(lambda: shown() == ["measuring"], "a measure under way says so")
    if look.stacked:
        assert not descendant(page, "emptyLabel").isVisible(), "and only in the list"
    trace = FocusTrace(look.window)
    gate.set()
    until(lambda: shown() == ["retry"], "a failed measure offers its retry")
    until(lambda: cards in focus_names(look.window))
    settle(look.window)
    assert trace.stop().foreign(cards) == [], "the focus stays in the rows from measuring to the retry"
    if look.stacked:
        assert not descendant(page, "emptyLabel").isVisible()
    failing.clear()
    trace = FocusTrace(look.window)
    look.press(Qt.Key.Key_Return)
    until(lambda: "root:games" in shown(), "A measures again")
    settle(look.window)
    assert trace.stop().foreign(cards) == [], "the focus stays in the rows while they swap"
    assert "retry" not in shown()
    if look.stacked:
        return

    rows = descendant(page, "settingsCards")
    look.press(Qt.Key.Key_Down, 3)
    held_key = read(rows, "currentRow")["key"]
    gate.clear()
    failing.append(True)
    look.press(Qt.Key.Key_I)
    until(lambda: api.screens.storage.loading, "X measures again")
    assert "measuring" not in shown(), "the rows stay while they are measured again"
    assert read(rows, "currentRow")["key"] == held_key, "and the cursor with them"
    trace = FocusTrace(look.window)
    gate.set()
    until(lambda: "retry" in shown(), "a failed measure over the rows offers its retry")
    until(lambda: read(rows, "currentRow")["key"] == "retry", "the cursor goes to it")
    assert read(rows, "caption") == "the games folder could not be read", "and shows why"
    settle(look.window)
    assert trace.stop().foreign("settingsCards") == [], "the focus stays in the cards as the retry comes in over the rows"

    failing.clear()
    look.press(Qt.Key.Key_Down, 2)
    held_key = read(rows, "currentRow")["key"]
    look.press(Qt.Key.Key_I)
    until(lambda: "retry" not in shown() and not api.screens.storage.loading, "a measure that succeeds takes the retry away")
    settle(look.window)
    assert read(rows, "currentRow")["key"] == held_key, "the cursor keeps its row, not its index"

    failing.append(True)
    look.press(Qt.Key.Key_I)
    until(lambda: read(rows, "currentRow")["key"] == "retry")
    failing.clear()
    while not read(rows, "currentRow")["key"].startswith("trash:"):
        look.press(Qt.Key.Key_Down)
    trashed, after = shown()[read(rows, "index") : read(rows, "index") + 2]
    look.press(Qt.Key.Key_Return)
    dialog = page.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True, "the trash asks first")
    look.press(Qt.Key.Key_Down)
    until(lambda: dialog.property("index") == 1)
    look.press(Qt.Key.Key_Return)
    until(lambda: trashed not in shown() and "retry" not in shown() and not api.screens.storage.loading, "the trash measures again")
    settle(look.window)
    assert read(rows, "currentRow")["key"] == after, "the cursor takes the next row, not the one two below"

    gate.clear()
    failing.append(True)
    look.press(Qt.Key.Key_I)
    until(lambda: api.screens.storage.loading)
    invoke(page, "land", "launch")
    until(lambda: read(page, "sectionId") == "launch" and "settingsCards" in focus_names(look.window))
    settle(look.window)
    look.press(Qt.Key.Key_Down, 2)
    elsewhere = read(rows, "currentRow")["key"]
    gate.set()
    until(lambda: api.screens.storage.error != "")
    settle(look.window)
    assert read(rows, "currentRow")["key"] == elsewhere and "settingsCards" in focus_names(look.window), "a failure on Storage leaves the section shown alone"


@pytest.mark.parametrize("look", ["switch2", "ps5"], indirect=True)
def test_a_console_storage_refresh_that_fails_over_its_rows_says_why_under_the_cursor(api, fake, look, monkeypatch):
    real, failing = fake.core.storage, []
    reason = "the games folder could not be read"

    def measured():
        if failing:
            raise UniverseError("Io", reason)
        return real()

    monkeypatch.setattr(fake.core, "storage", measured)
    page = look.open("pages/StoragePage.qml")

    def down_to(prefix):
        for _ in range(40):
            if under_cursor(page).get("key", "").startswith(prefix):
                return
            look.press(Qt.Key.Key_Down)
        raise AssertionError(f"no {prefix} row below the cursor")

    until(lambda: any(r["key"].startswith("trash:") for r in listed(look, page)) and under_cursor(page))
    down_to("trash:")
    failing.append(True)
    look.press(Qt.Key.Key_Return)
    dialog = look.dialog()
    until(lambda: dialog.property("open") is True, "the trash asks first")
    look.press(Qt.Key.Key_Right)
    until(lambda: dialog.property("index") == 1)
    look.press(Qt.Key.Key_Return)
    until(lambda: fake.core.trashed, "the leftover goes")
    until(lambda: under_cursor(page).get("key") == "retry", "the reload after it fails: its retry takes the cursor, the rows held")
    assert under_cursor(page)["detail"] == reason, "and says why"
    assert any(r["key"].startswith("game:") for r in listed(look, page))

    failing.clear()
    look.press(Qt.Key.Key_Return)
    until(lambda: "retry" not in [r["key"] for r in listed(look, page)] and not api.screens.storage.loading)
    down_to("game:")
    look.press(Qt.Key.Key_Return)
    until(lambda: "dataPage" in focus_names(look.window), "a game's row opens its Data page")
    failing.append(True)
    look.press(Qt.Key.Key_Escape)
    until(lambda: under_cursor(page).get("key") == "retry", "back on Storage, the reload fails: its retry takes the cursor")
    assert under_cursor(page)["detail"] == reason
