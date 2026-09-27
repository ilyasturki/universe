import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_render import render

from conftest import pump, wait_for


@pytest.fixture
def ps5(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root
    window.close()
    pump(50)
    del engine


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def open_store(root, args=None):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/InstallPage.qml"), Q_ARG("QVariant", args or {}))
    pump(500)
    return root.property("topPage")


def titles(store, collection):
    coll = next(c for c in value(store, "collections") if c["id"] == collection)
    listed = store.property("sources").property("rows")
    return [listed[int(i)]["title"] for i in coll["items"]]


def test_the_store_lays_the_catalogue_out_in_collections(ps5, api):
    _window, root = ps5
    store = open_store(root)
    assert store.property("strip") is False, "the console has a store: no hint strip"
    assert [c["title"] for c in value(store, "collections")] == ["Ready to install", "Updates", "Installed"]
    assert titles(store, "ready") == ["Disco Elysium", "Stardew Valley", "The Witcher 3: Wild Hunt"]
    assert titles(store, "updates") == ["Mini Metro"]
    assert "The Technomancer" in titles(store, "installed")
    assert [line["label"] for line in value(store, "lines") if line.get("heading")] == ["Installing", "Updates", "Installed"]
    assert value(store, "current")["title"] == "Disco Elysium" and value(store, "tabs") == ["GOG", "Downloads"]


def test_the_cursor_walks_the_collections_and_a_opens_the_game_card(ps5):
    window, root = ps5
    store = open_store(root)
    QTest.keyClick(window, Qt.Key.Key_Right)
    pump(50)
    assert value(store, "current")["title"] == "Stardew Valley"
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(50)
    assert store.property("row") == 1 and value(store, "current")["title"] == "Mini Metro"
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(100)
    assert store.property("zone") == "card" and value(store, "cardEntry")["title"] == "Mini Metro"
    assert [a["id"] for a in value(store, "cardActions")][:2] == ["update", "play"], "an update waits: Update first"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(50)
    assert store.property("zone") == "main" and root.property("depth") == 1, "B closes the card, not the page"
    QTest.keyClick(window, Qt.Key.Key_Up)
    QTest.keyClick(window, Qt.Key.Key_Up)
    pump(50)
    assert store.property("zone") == "top"


def test_a_search_narrows_the_store_to_its_results_and_b_clears_it(ps5, api):
    window, root = ps5
    sources = api.screens.sources
    store = open_store(root)
    sources.search("witcher")
    wait_for(sources.rowsChanged, 3000)
    pump(100)
    collections = value(store, "collections")
    assert len(collections) == 1 and collections[0]["title"] == "Results for “witcher”"
    found = titles(store, "results")
    assert len(found) == len(sources.rows) and all("Witcher" in t for t in found)
    assert "The Witcher: Enhanced Edition" in found and value(store, "current")["title"] == found[0]
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(200)
    assert sources.query == "" and root.property("depth") == 1, "B drops the search first"
    assert next(c["title"] for c in value(store, "collections")) == "Ready to install"


def test_an_install_runs_as_a_job_heads_the_home_row_and_x_stops_it(ps5, api, fake):
    window, root = ps5
    sources = api.screens.sources
    store = open_store(root)
    QTest.keyClick(window, Qt.Key.Key_Right)
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(100)
    assert value(store, "cardActions")[0]["id"] == "install"
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(100)
    assert root.property("modal") is True, "an install asks first, with the sizes"
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(300)
    job = sources.job
    assert job is not None and job["ok"] is None and job["title"] == "Stardew Valley"
    assert "X" in [h["glyph"] for h in value(store, "hints")], "X cancels while a job runs"
    assert sources.arriving is not None and sources.arriving.property("installing")
    home = root.findChild(QObject, "homePage")
    entries = value(home, "gameEntries")
    assert entries[1]["game"].property("id") == sources.arriving.property("id"), "the download heads the home row"
    other = next(i for i, r in enumerate(sources.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    said = []
    sources.message.connect(said.append)
    assert sources.install(other) == "" and said == ["Installing Stardew Valley first — cancel it or wait"]
    QTest.keyClick(window, Qt.Key.Key_I)
    wait_for(fake.jobFinished, 5000)
    pump(300)
    assert sources.job["cancelled"] and next(r for r in sources.rows if r["title"] == "Stardew Valley")["partial"]


def test_downloads_opens_with_tab_one_and_the_bumpers_switch_tabs(ps5, api):
    window, root = ps5
    store = open_store(root, {"tab": 1})
    assert store.property("tab") == 1
    lines = value(store, "lines")
    assert [line["label"] for line in lines if line.get("heading")] == ["Installing", "Updates", "Installed"]
    assert value(store, "line")["game"]["title"] == "Disco Elysium", "the cursor on the first line under its heading"
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(50)
    assert value(store, "line").get("all") is True, "Update everything under the Updates heading"
    QTest.keyClick(window, Qt.Key.Key_Q)
    pump(50)
    assert store.property("tab") == 0
    QTest.keyClick(window, Qt.Key.Key_E)
    pump(50)
    assert store.property("tab") == 1


def test_a_second_store_puts_a_switch_on_top_whose_menu_lists_the_stores(ps5, api, fake):
    window, root = ps5
    fake.core._source("epic").update(enabled=True, logged_in=True)
    fake.core._data["source_library"]["epic"] = [{"id": "Min", "title": "Hades", "owned": True, "installed": False}]
    sources = api.screens.sources
    store = open_store(root)

    def until(done):
        for _ in range(60):
            pump(50)
            if done():
                return
        raise AssertionError("never came")

    until(lambda: len(value(store, "icons")) == 5)
    assert [i["id"] for i in value(store, "icons")] == ["store", "search", "refresh", "sources", "options"]
    QMetaObject.invokeMethod(store, "openStores")
    pump(100)
    popup = root.findChild(QObject, "popup")
    assert popup.property("open") is True and [i["label"] for i in value(popup, "items")] == ["GOG", "Epic Games"]
    QTest.keyClick(window, Qt.Key.Key_Down)
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: [r["title"] for r in sources.rows] == ["Hades"])
    assert sources.source == "epic" and value(store, "tabs") == ["Epic Games", "Downloads"]


def test_start_opens_every_action_in_a_menu(ps5):
    window, root = ps5
    store = open_store(root)
    QTest.keyClick(window, Qt.Key.Key_F1)
    pump(100)
    popup = root.findChild(QObject, "popup")
    assert popup.property("open") is True and root.property("modal") is True
    labels = [i["label"] for i in value(popup, "items")]
    assert labels[0] == "Resume", "the focused game's action first: Disco Elysium is paused"
    assert {"Search…", "Update Everything", "Downloads", "Refresh", "Sources Settings"} <= set(labels)
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(100)
    assert popup.property("open") is False and store.property("zone") == "main"
