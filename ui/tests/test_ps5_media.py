import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, Qt
from PySide6.QtTest import QTest
from test_render import render

from conftest import until

MINE = ("MediaGalleryPage", "PlayerPage", "NewsPage", "ArticlePage", "Gallery", "JournalCard")


@pytest.fixture
def ps5(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root
    assert [w for w in warnings if any(name in w for name in MINE)] == []
    window.close()
    del engine


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def push(root, source, args):
    depth = root.property("depth")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    until(lambda: root.property("depth") == depth + 1 and root.property("topPage").property("activeFocus"))
    return root.property("topPage")


def key(window, k, times=1):
    for _ in range(times):
        QTest.keyClick(window, k)


def gallery(api, root, args=None):
    page = push(root, "pages/MediaGalleryPage.qml", args or {})
    until(lambda: not api.screens.media.loading and api.screens.media.rows)
    return page


def test_the_gallery_holds_every_kind_and_each_tab_one(api, ps5):
    window, root = ps5
    page = gallery(api, root)
    shown = value(page, "shown")
    assert {r["kind"] for r in shown} == {"shot", "recording", "journal"} and len(shown) == len(api.screens.media.rows)
    assert [r["when"] for r in shown] == sorted((r["when"] for r in shown), reverse=True), "newest first"
    for tab, kind in ((1, "shot"), (2, "recording"), (3, "journal")):
        key(window, Qt.Key.Key_E)
        assert page.property("tab") == tab
        kinds = {r["kind"] for r in value(page, "shown")}
        assert kinds == {kind}
    key(window, Qt.Key.Key_Q, 3)
    assert page.property("tab") == 0


def test_a_game_named_filters_the_gallery(api, ps5):
    _window, root = ps5
    page = gallery(api, root, {"gameId": "the-technomancer"})
    shown = value(page, "shown")
    assert shown and all(r["gameId"] == "the-technomancer" for r in shown) and page.property("filterId") == "the-technomancer"
    root.pop()
    other = gallery(api, root, {"gameId": "dead-cells"})
    assert value(other, "shown") == [], "a game with no captures shows none"


def test_a_shot_opens_full_screen_and_steps_through_the_shots(api, ps5):
    window, root = ps5
    page = gallery(api, root)
    key(window, Qt.Key.Key_E)
    key(window, Qt.Key.Key_Return)
    assert page.property("viewing") is True and page.property("shotIndex") == 0
    key(window, Qt.Key.Key_Right)
    assert page.property("shotIndex") == 1 and page.property("index") == 1, "the grid follows the picture"
    key(window, Qt.Key.Key_Escape)
    assert page.property("viewing") is False and root.property("depth") == 1, "B closes the picture, not the page"


def test_a_recording_opens_the_player_which_finds_its_row_cold(api, ps5):
    window, root = ps5
    api.screens.album.load("dead-cells")
    page = gallery(api, root)
    key(window, Qt.Key.Key_E, 2)
    session = value(page, "current")["session"]
    key(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 2)
    player = root.property("topPage")
    assert player.property("strip") is True
    row = until(lambda: value(player, "row"), "the album was asked for the game's rows")
    assert row["session"] == session, "the album was asked for the game's rows"
    assert api.screens.album.gameId == "the-technomancer"
    until(lambda: player.property("activeFocus"))
    key(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == 1)


def test_a_journal_row_opens_its_article(api, ps5):
    window, root = ps5
    page = gallery(api, root)
    key(window, Qt.Key.Key_E, 3)
    session = value(page, "current")["session"]
    key(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 2)
    row = until(lambda: value(root.property("topPage"), "row"))
    assert row["session"] == session and row["state"] == "written"


def test_a_capture_named_by_the_hub_opens_directly(api, ps5):
    _window, root = ps5
    shots = [r for r in api.universe.media("the-technomancer") if r["kind"] == "shot"]
    page = gallery(api, root, {"gameId": "the-technomancer", "path": shots[1]["path"], "session": shots[1]["session"]})
    until(lambda: page.property("viewing") is True, "a shot opens full screen")
    assert value(page, "current")["path"] == shots[1]["path"], "a shot opens full screen"
    root.pop()
    recording = next(r for r in api.universe.media("the-technomancer") if r["kind"] == "recording")
    QMetaObject.invokeMethod(
        root,
        "push",
        Q_ARG("QVariant", "pages/MediaGalleryPage.qml"),
        Q_ARG("QVariant", {"gameId": "the-technomancer", "path": recording["path"], "session": recording["session"]}),
    )
    until(lambda: root.property("depth") == 2, "a recording opens in the player")
    row = until(lambda: value(root.property("topPage"), "row"), "a recording opens in the player")
    assert row["session"] == recording["session"], "a recording opens in the player"


def test_an_article_pushed_cold_loads_its_game(api, ps5):
    _window, root = ps5
    api.screens.news.load("dead-cells")
    article = push(root, "pages/ArticlePage.qml", {"gameId": "the-technomancer", "session": "20260907-224100"})
    row = until(lambda: value(article, "row"))
    assert row["title"] == "Ophir, first night" and row["next_up"] != ""
    assert api.screens.news.gameId == "the-technomancer"


def test_the_journal_lists_every_session_and_its_state(api, ps5):
    window, root = ps5
    page = push(root, "pages/NewsPage.qml", {})
    entries = until(lambda: value(page, "entries"))
    assert [e["state"] for e in entries] == ["written", "written", "none", "deferred"]
    key(window, Qt.Key.Key_F1)
    assert root.property("modal") is True, "Start opens the entry's menu"
    key(window, Qt.Key.Key_Escape)
    assert root.property("modal") is False
    key(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 2)
    row = until(lambda: value(root.property("topPage"), "row"))
    assert row["session"] == entries[0]["session"]
