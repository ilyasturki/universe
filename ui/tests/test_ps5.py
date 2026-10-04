from pathlib import Path

import pytest
from looks import Look, call, invoke, page_name, read
from PySide6.QtCore import QObject, Qt, QUrl
from PySide6.QtTest import QTest

from conftest import record, until

QML = Path(__file__).resolve().parents[1] / "universe_ui" / "qml"


@pytest.fixture
def ps5(api):
    shown = Look(api, "ps5")
    yield shown.window, shown.root
    shown.close()


def test_the_row_opens_on_the_last_game_with_the_welcome_hub_before_it(ps5):
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    kinds = [e["kind"] for e in read(home, "gameEntries")]
    assert kinds[0] == "welcome" and kinds[-3:] == ["store", "gallery", "library"]
    assert home.property("index") == 1 and home.property("currentGame").property("id") == "the-technomancer"
    assert home.property("heroShown") is True and home.property("zone") == "rail"


def test_a_move_drops_the_hero_and_it_comes_back_once_the_row_rests(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    QTest.keyClick(window, Qt.Key.Key_Right)
    assert home.property("heroShown") is False and home.property("titleShown") is False, "the hero and the name go at once"
    moved = home.property("currentGame").property("id")
    until(lambda: home.property("heroShown") is True)
    assert home.property("sideShown") is False, "the side tile comes last"
    until(lambda: home.property("sideShown") is True)
    assert read(home, "rested")["game"].property("id") == moved


def test_down_goes_into_the_hero_then_the_hub_which_hides_the_strips_above_and_b_goes_back_up(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    scrim = root.findChild(QObject, "hubScrim")
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "hero" and home.property("scroll") > 0)
    assert scrim.property("visible") is False
    opened = {c["open"]["page"] for s in read(home, "strips") for c in s["cards"] if "open" in c}
    assert {"pages/PlayLogPage.qml", "pages/MediaGalleryPage.qml", "pages/SoftwareInfoPage.qml"} <= opened
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "hub" and home.property("strip") == 0)
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("strip") == 1)
    hub = root.findChild(QObject, "hub")
    strips = [s for s in hub.childItems() if s.objectName() == "hubStrip"]
    until(lambda: strips[0].property("opacity") == 0 and scrim.property("opacity") == 1)
    assert strips[1].property("opacity") == 1 and strips[2].property("opacity") == 1
    assert strips[0].property("enabled") is False, "and a tap where it hides picks nothing"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: home.property("zone") == "rail" and home.property("scroll") == 0)


def test_the_hero_never_shows_the_plain_title_while_a_logo_loads(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    title = root.findChild(QObject, "heroTitle")
    logo = root.findChild(QObject, "heroLogo")
    shown = record(title.visibleChanged)
    for _ in range(2):
        QTest.keyClick(window, Qt.Key.Key_Right)
        until(lambda: home.property("heroShown") is True and logo.property("progress") == 1)
    assert title.property("visible") is False and not shown


def test_the_hero_side_tile_takes_a_screenshot_before_the_banner(ps5):
    _window, root = ps5
    game = root.findChild(QObject, "homePage").property("currentGame")
    side = root.findChild(QObject, "homeHero").property("sideArt").toString()
    shots = [s.toString() if isinstance(s, QUrl) else QUrl.fromLocalFile(s).toString() for s in game.property("assets").property("screenshotList")]
    assert side in shots, "the fixture's games have a banner too"


def hub_of(home, **game):
    blank = {"playCount": 0, "playTime": 0, "achievementsTotal": 0, "description": "", "summary": "", "players": 0, "source": ""}
    lists = {"developerList": [], "genreList": [], "publisherList": [], "assets": {"screenshotList": [], "background": "", "banner": "", "boxFront": ""}}
    return call(home, "hubFor", {**blank, **lists, **game})


def card(strips, badge):
    return next(c for s in strips for c in s["cards"] if c.get("badge") == badge)


def test_the_hub_s_cards_have_no_stray_separator_and_never_repeat_the_game_s_title(ps5):
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    game = {"id": "the-technomancer", "title": "The Technomancer"}
    strips = hub_of(home, **game, playTime=15 * 3600, genreList=["RPG"])
    body = card(strips, "clock")["body"]
    assert body and not body.startswith("·") and not body.startswith(" "), "no session count, no separator before the hours"
    assert card(strips, "info")["title"] != game["title"]
    assert card(hub_of(home, **game, publisherList=["Focus"]), "info")["title"] == "Focus"


def test_left_of_the_first_game_is_the_welcome_hub_whose_latest_recording_shows_its_game_s_art(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    hub = root.findChild(QObject, "welcomeHub")
    QTest.keyClick(window, Qt.Key.Key_Left)
    until(lambda: read(home, "rested")["kind"] == "welcome" and hub.property("shown") is True and read(hub, "latest"))
    latest = read(hub, "latest")
    assert latest["kind"] == "recording" and latest["image"] == "", "the fake core has no frames to extract"
    assert hub.property("latestImage") != ""
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "welcome")
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: home.property("zone") == "rail")


def test_glyphs_are_the_dualsense_s_until_a_pad_is_seen_then_that_pad_s(ps5, api):
    from universe_ui.screens.controller import FakeWatcher

    _window, root = ps5
    invoke(root, "push", "pages/InstallPage.qml", {})

    # The page's hint box rebuilds its glyphs as it loads: the families are read in one pass.
    def families():
        page = root.property("topPage")
        return {g.property("family") for g in page.findChildren(QObject, "hintGlyph")} if page else set()

    assert until(families) == {"dualsense"}
    screen = api.screens.controller
    screen.restart_ms = 0
    screen.start(FakeWatcher("xbox"))
    until(lambda: families() == {"xbox"})


def evaluate(imports, expression):
    from PySide6.QtQml import QQmlComponent, QQmlEngine

    engine = QQmlEngine()
    component = QQmlComponent(engine)
    lines = "".join(f'import "{(QML / path).as_uri()}" as {name}\n' for name, path in imports.items())
    component.setData(f"import QtQuick\n{lines}QtObject {{ property var out: {expression} }}\n".encode(), QUrl("file:///evaluate.qml"))
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    return obj.property("out").toVariant()


def test_copy_names_the_button_the_pad_carries(app):
    names = evaluate({"Names": "ui/PadNames.js"}, "['dualsense', 'xbox', 'switch-pro'].map(f => [Names.buttonName('A', f), Names.buttonName('X', f)])")
    assert names == [["✕", "□"], ["A", "X"], ["A", "X"]]


def test_a_trophy_s_unlock_date_is_relative_and_falls_back_to_the_store_s(app):
    days_ago = "new Date(Date.now() - 3 * 86400000)"
    earned, played, unknown, rarity, unreadable, placeholder = evaluate(
        {"Trophy": "ps5/ui/Trophy.js", "Format": "core/Format.js"},
        f"[Trophy.earned({days_ago}.toISOString()), Format.lastPlayed({days_ago}), Trophy.earned(''), Trophy.rarity(-1), "
        "Trophy.earned('1727432880', 'shown'), Trophy.earned('1970-01-01T00:00:00Z', 'shown')]",
    )
    assert earned == played and unknown == "" and rarity == ""
    assert unreadable == "shown" and placeholder == "shown", "a time JS cannot read, or a placeholder, shows the store's own date"


def test_a_menu_row_with_a_detail_is_as_tall_as_its_text(ps5):
    _window, root = ps5
    popup = root.findChild(QObject, "popup")
    invoke(root, "showMenu", {"items": [{"label": "One line", "detail": "Short."}, {"label": "Long", "detail": "Words " * 40}]}, None)
    until(lambda: len(read(popup, "detailHeights")) == 2)
    row, one, three = popup.property("rowHeight"), call(popup, "heightOf", 0), call(popup, "heightOf", 1)
    assert row < one < 2 * row, "one line of detail, no empty band under it"
    assert three > one + row / 2, "three lines take their room"


def test_the_power_menu_s_way_out_asks_first_while_a_game_runs(ps5, api):
    from PySide6.QtQml import qmlEngine

    window, root = ps5
    popup = root.findChild(QObject, "popup")
    dialog = root.findChild(QObject, "dialog")
    engine = qmlEngine(root)
    engine.quit.disconnect()
    quits = record(engine.quit)
    invoke(root, "launch", api.allGames.byId("dead-cells"))
    until(lambda: api.home.shown == "game")

    def way_out():
        invoke(root, "askPower")
        until(lambda: popup.property("open") is True)
        acts = [i["act"] for i in read(popup, "items")]
        for _ in range(len(acts) - 1):
            QTest.keyClick(window, Qt.Key.Key_Down)
        assert acts[popup.property("index")] in ("quit", "logout")
        QTest.keyClick(window, Qt.Key.Key_Return)
        until(lambda: dialog.property("open") is True, "the game under Home would be closed")

    way_out()
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: dialog.property("open") is False)
    assert api.universe.currentSession, "Cancel leaves the game running"
    way_out()
    QTest.keyClick(window, Qt.Key.Key_Right)
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: api.universe.currentSession is None, "Quit closes the game it said it would")
    until(lambda: quits, "then Universe quits")


def test_every_tab_bar_boxes_its_focused_tab_the_same_way(ps5):
    window, root = ps5
    for source in ["", "pages/LibraryPage.qml", "pages/InstallPage.qml", "pages/MediaGalleryPage.qml"]:
        page = root.findChild(QObject, "homePage")
        if source:
            invoke(root, "push", source, {})
            name = page_name(source)
            page = until(lambda name=name: (top := root.property("topPage")) is not None and top.objectName() == name and top.property("activeFocus") and top)
        for _ in range(3):
            QTest.keyClick(window, Qt.Key.Key_Up)
        bar = until(lambda page=page: page.findChild(QObject, "tabBar"), source)
        tabs = [t for t in bar.childItems() if t.objectName() == "tabLabel"]
        until(lambda tabs=tabs: any(t.property("focused") for t in tabs), source)
        assert len(tabs) >= 2 and sum(t.property("focused") for t in tabs) == 1, source
