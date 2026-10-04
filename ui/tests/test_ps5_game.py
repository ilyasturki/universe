import pytest
from looks import Look
from looks import read as value
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest

from conftest import record, until

PAGES = (
    "AchievementsPage",
    "SoftwareInfoPage",
    "GameSettingsPage",
    "ArtworkPage",
    "ArtworkSlotPage",
    "PlayLogPage",
    "SessionLogPage",
    "GameCellGrid",
    "GameShotViewer",
    "TrophyCard",
)


@pytest.fixture
def ps5(api):
    shown = Look(api, "ps5")
    warnings = record(shown.engine.warnings)
    yield shown.window, shown.root
    mine = [w.toString() for (ws,) in warnings for w in ws if any(p + ".qml" in w.toString() for p in PAGES)]
    shown.close()
    assert mine == [], "no QML warning from the game pages"


def push(root, source, args):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    return until(lambda: (top := root.property("topPage")) is not None and top.property("activeFocus") and top)


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)


def test_the_trophies_list_the_game_s_achievements_open_one_whole_and_sort_them(ps5, api):
    window, root = ps5
    store = api.screens.achievements
    page = push(root, "pages/AchievementsPage.qml", {"gameId": "batman-arkham-origins"})
    until(lambda: store.count == 6 and len(value(page, "rows")) == 6)
    rows = value(page, "rows")
    assert page.property("strip") is False and page.property("progress") == 50
    assert [r["unlocked"] for r in rows] == [True, True, True, False, False, False], "earned first, as the store orders them"
    assert rows[-1]["key"] == "hidden" and rows[-1]["hidden"] == 1, "a hidden one keeps its secret"
    dialog = root.findChild(QObject, "dialog")
    click(window, Qt.Key.Key_Return)
    until(lambda: dialog.property("open") is True)
    assert dialog.property("message") == rows[0]["name"] and rows[0]["description"] in dialog.property("detail")
    click(window, Qt.Key.Key_Return)
    until(lambda: dialog.property("open") is False and page.property("activeFocus"))
    click(window, Qt.Key.Key_End)
    until(lambda: page.property("index") == 5)
    click(window, Qt.Key.Key_Return)
    assert dialog.property("open") is False, "a hidden trophy keeps its secret"
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    checked = [i.get("check") is True for i in value(popup, "items")]
    assert checked == [o["id"] == "default" for o in value(page, "orders")] + [False], "every order, the one shown checked, then a refresh"
    click(window, Qt.Key.Key_Down)
    click(window, Qt.Key.Key_Return)
    until(lambda: page.property("order") == "rare")
    rows = value(page, "rows")
    rarities = [r["rarity"] for r in rows[:-1]]
    assert rarities == sorted(rarities) and rows[-1]["key"] == "hidden" and page.property("index") == 0, "the hidden ones stay last"
    click(window, Qt.Key.Key_I)
    until(lambda: not store.loading)
    assert store.count == 6, "X asks the store again"


def test_the_information_opens_a_screenshot_full_screen(ps5, api):
    window, root = ps5
    depth = root.property("depth")
    page = push(root, "pages/SoftwareInfoPage.qml", {"gameId": "batman-arkham-origins", "shot": 1})
    assert not any(f["value"].endswith(".exe") for f in until(lambda: value(page, "facts"))), "a store game: no program yet"
    until(lambda: page.property("viewerOpen") is True and page.property("zone") == "shots", "args.shot opens that screenshot")
    click(window, Qt.Key.Key_Left)
    click(window, Qt.Key.Key_Escape)
    until(lambda: page.property("viewerOpen") is False and page.property("shotIndex") == 0, "B closes the viewer on the shot it showed")
    assert root.property("depth") == depth + 1, "and only the viewer"
    click(window, Qt.Key.Key_Return)
    until(lambda: page.property("viewerOpen") is True)
    click(window, Qt.Key.Key_Return)
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    assert [i["act"] for i in value(popup, "items")][:3] == ["play", "shots", "trophies"]
    click(window, Qt.Key.Key_Down, 2)
    click(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == depth + 2 and root.property("topPage").objectName() == "achievements", "Options reaches the trophies")
    click(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == depth + 1)
    game = api.allGames.byId("the-technomancer")
    page = push(root, "pages/SoftwareInfoPage.qml", {"gameId": game.id})
    shown = [f["value"] for f in until(lambda: value(page, "facts"))]
    assert any(v.endswith("TheTechnomancer.exe") for v in shown), "where it lives on disk"
    assert str(game.property("playCount")) in shown, "how often it was played"


def test_a_game_s_pages_title_with_their_name_beside_the_game_s_tile(ps5, api):
    _window, root = ps5
    game = api.allGames.byId("the-technomancer")
    sources = ["SoftwareInfoPage", "AchievementsPage", "GameSettingsPage", "NewsPage", "MediaGalleryPage", "PlayLogPage", "ArtworkPage", "DataPage"]
    for depth, source in enumerate(sources, start=1):
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", f"pages/{source}.qml"), Q_ARG("QVariant", {"gameId": game.id}))
        until(lambda depth=depth: root.property("depth") == depth)
        page = root.property("topPage")
        header = until(lambda page=page: page.findChild(QObject, "pageTitle"), source)
        assert header.property("game").property("id") == game.id, source
        assert header.property("title") not in ("", game.title), f"{source}: the page's name, the game is the tile"


def test_the_play_log_lists_the_sessions_and_reads_one(ps5, api):
    window, root = ps5
    store = api.screens.sessions
    page = push(root, "pages/PlayLogPage.qml", {"gameId": "the-technomancer"})
    depth = root.property("depth")
    until(lambda: store.count > 0 and len(value(page, "entries")) == store.count)
    entries = value(page, "entries")
    assert page.property("strip") is False
    crashed = {r["session"] for r in store.rows if r["bad"]}
    assert crashed and {e["icon"] for e in entries if e["session"] in crashed} == {"warning"}, "a crash wears a warning"
    click(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == depth + 1)
    log = root.property("topPage")
    assert log.property("session") == entries[0]["session"] and log.property("strip") is True
    until(lambda: len(store.log) > 0 and not store.logLoading)
    click(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == depth)
    page = push(root, "pages/PlayLogPage.qml", {"gameId": "batman-arkham-origins"})
    until(lambda: [e["key"] for e in value(page, "entries")] == ["none"], "never played: one still row says so")
    click(window, Qt.Key.Key_Return)
    assert root.property("topPage") == page, "and it opens nothing"
