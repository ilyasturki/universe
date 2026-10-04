from pathlib import Path

import pytest
from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QObject, QUrl
from PySide6.QtQml import QQmlComponent, QQmlEngine
from test_render import render

from conftest import record, until


def opened(root, name):
    top = root.property("topPage")
    return (top is not None and top.objectName() == name) or root.findChild(QObject, name) is not None


def keys(store):
    return [r["key"] for r in store.rows if r["key"]]


def notices(engine):
    core = Path(__file__).resolve().parents[1] / "universe_ui" / "qml" / "core"
    component = QQmlComponent(engine, engine)
    component.setData(
        f'import QtQuick\nimport "{core.as_uri()}"\nQtObject {{ function current() {{ return Notices.current; }} }}'.encode(), QUrl("file:///probe.qml")
    )
    probe = component.create()
    assert probe is not None, [e.toString() for e in component.errors()]
    engine.setObjectOwnership(probe, QQmlEngine.ObjectOwnership.CppOwnership)

    def current():
        out = QMetaObject.invokeMethod(probe, "current", Q_RETURN_ARG("QVariant"))
        return out.toVariant() if hasattr(out, "toVariant") else out

    return current


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


@pytest.mark.parametrize("look", ["reprise", "switch2", "ps5"])
def test_every_look_opens_a_games_data_page_and_the_storage_view(api, fake, look):
    api.theme.set(look)
    api.theme.takeLanding()
    engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    if look == "reprise":
        root.openSub("pages/DataPage.qml", {"game": api.allGames.byId("the-technomancer")})
    else:
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/DataPage.qml"), Q_ARG("QVariant", {"gameId": "the-technomancer"}))
    until(lambda: opened(root, "dataPage"), "the Data page opens")
    store = api.screens.gameData
    until(lambda: store.gameId == "the-technomancer" and "backup" in keys(store))
    fake.core.failing_tools["winetricks"] = "winetricks failed with exit status 127"
    shown = notices(engine)
    assert store.act("winetricks") is True
    failed = until(lambda: (n := shown()) and n.get("error") and n)
    assert failed["text"] == fake.core.failing_tools["winetricks"], "the failure and its reason, not a start"
    if look == "reprise":
        root.openSettings("storage")
        until(lambda: (page := root.property("activePage")) is not None and page.property("sectionId") == "storage")
    else:
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/StoragePage.qml"), Q_ARG("QVariant", {}))
        until(lambda: opened(root, "storagePage"), "the Storage view opens")
    until(lambda: "root:games" in keys(api.screens.storage))
    window.close()
