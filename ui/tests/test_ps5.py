import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt, QUrl
from PySide6.QtTest import QTest
from test_render import render

from conftest import record, until


@pytest.fixture
def ps5(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root
    window.close()
    del engine


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def test_the_row_opens_on_the_last_game_with_the_welcome_hub_before_it(ps5):
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    kinds = [e["kind"] for e in value(home, "gameEntries")]
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
    assert value(home, "rested")["game"].property("id") == moved


def test_down_goes_into_the_hero_then_the_hub_and_b_goes_back_up(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "hero" and home.property("scroll") > 0)
    strips = [s["title"] for s in value(home, "strips")]
    assert strips[0] == "Continue where you left off" and "Captures" in strips and "About" in strips
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "hub" and home.property("strip") == 0)
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: home.property("zone") == "rail" and home.property("scroll") == 0)


def test_the_hero_never_shows_the_plain_title_while_a_logo_loads(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    title = root.findChild(QObject, "heroTitle")
    logo = root.findChild(QObject, "heroLogo")
    shown = record(title.visibleChanged)
    for _ in range(3):
        QTest.keyClick(window, Qt.Key.Key_Right)
        until(lambda: home.property("heroShown") is True and logo.property("progress") == 1)
    assert title.property("visible") is False and not shown


def test_the_hero_side_tile_takes_a_screenshot_before_the_banner(ps5):
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    game = home.property("currentGame")
    side = root.findChild(QObject, "homeHero").property("sideArt").toString()
    shots = [s.toString() if isinstance(s, QUrl) else QUrl.fromLocalFile(s).toString() for s in game.property("assets").property("screenshotList")]
    assert side in shots, "the fixture's games have a banner too"


def test_the_hub_hides_the_strips_above_and_fades_under_the_header(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    scrim = root.findChild(QObject, "hubScrim")
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "hero")
    assert scrim.property("visible") is False
    QTest.keyClick(window, Qt.Key.Key_Down)
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("strip") == 1)
    hub = root.findChild(QObject, "hub")
    strips = [s for s in hub.childItems() if s.objectName() == "hubStrip"]
    until(lambda: strips[0].property("opacity") == 0 and scrim.property("opacity") == 1)
    assert strips[1].property("opacity") == 1 and strips[2].property("opacity") == 1


def hub_of(home, **game):
    from PySide6.QtCore import Q_RETURN_ARG

    blank = {"playCount": 0, "playTime": 0, "achievementsTotal": 0, "description": "", "summary": "", "players": 0, "source": ""}
    lists = {"developerList": [], "genreList": [], "publisherList": [], "assets": {"screenshotList": [], "background": "", "banner": "", "boxFront": ""}}
    return QMetaObject.invokeMethod(home, "hubFor", Q_RETURN_ARG("QVariant"), Q_ARG("QVariant", {**blank, **lists, **game})).toVariant()


def test_the_hub_s_last_session_card_has_no_stray_separator_without_a_session_count(ps5):
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    strips = hub_of(home, id="the-technomancer", title="The Technomancer", playTime=15 * 3600)
    body = strips[0]["cards"][0]["body"]
    assert body and not body.startswith("·") and not body.startswith(" ")


def test_the_hub_s_facts_card_never_repeats_the_game_s_title(ps5):
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    about = next(s for s in hub_of(home, id="the-technomancer", title="The Technomancer", genreList=["RPG"]) if s["title"] == "About")
    facts = next(c for c in about["cards"] if c.get("badge") == "info")
    assert facts["title"] != "The Technomancer"
    about = next(s for s in hub_of(home, id="the-technomancer", title="The Technomancer", publisherList=["Focus"]) if s["title"] == "About")
    assert next(c for c in about["cards"] if c.get("badge") == "info")["title"] == "Focus"


def test_left_of_the_first_game_is_the_welcome_hub(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    QTest.keyClick(window, Qt.Key.Key_Left)
    until(lambda: value(home, "rested")["kind"] == "welcome")
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "welcome")
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: home.property("zone") == "rail")


def test_the_welcome_hub_s_latest_recording_without_a_frame_shows_its_game_s_art(ps5, api):
    window, root = ps5
    hub = root.findChild(QObject, "welcomeHub")
    QTest.keyClick(window, Qt.Key.Key_Left)
    until(lambda: hub.property("shown") is True and value(hub, "latest"))
    latest = value(hub, "latest")
    assert latest["kind"] == "recording" and latest["image"] == "", "the fake core has no frames to extract"
    assert hub.property("latestImage") != ""


def test_back_from_a_game_the_home_builds_itself_up_again(api, fake):
    api.theme.set("ps5")
    api.theme.takeLanding()
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = root.findChild(QObject, "homePage")
    ended = record(fake.sessionEnded)
    QMetaObject.invokeMethod(root, "launch", Q_ARG("QVariant", api.allGames.byId("dead-cells")))
    until(lambda: api.home.shown == "game")
    assert home.property("railReveal") == 0, "blank under the game: nothing old flashes on the way back"
    fake.stop("")
    until(lambda: ended)
    assert home.property("railReveal") < 1 and home.property("heroShown") is False, "the row first, the hero later"
    until(lambda: home.property("railReveal") == 1 and home.property("chromeReveal") == 1 and home.property("heroShown") is True)
    assert home.property("currentGame").property("id") == "dead-cells", "the game just played, focused"
    window.close()


def test_glyphs_are_the_dualsense_s_until_a_pad_is_seen_then_that_pad_s(ps5, api):
    from universe_ui.screens.controller import FakeWatcher

    _window, root = ps5
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/InstallPage.qml"), Q_ARG("QVariant", {}))

    # The page's hint box rebuilds its glyphs as it loads: the families are read in one pass.
    def families():
        page = root.property("topPage")
        found = page.findChildren(QObject) if page else []
        return {o.property("family") for o in found if o.metaObject().className().startswith("HintGlyph")}

    assert until(families) == {"dualsense"}
    screen = api.screens.controller
    screen.restart_ms = 0
    screen.start(FakeWatcher("xbox"))
    until(lambda: families() == {"xbox"})


@pytest.mark.parametrize(("family", "names"), [("dualsense", ["✕", "□"]), ("xbox", ["A", "X"]), ("switch-pro", ["A", "X"])])
def test_copy_names_the_button_the_pad_carries(app, family, names):
    from pathlib import Path

    from PySide6.QtCore import QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine

    names_js = Path(__file__).resolve().parents[1] / "universe_ui" / "qml" / "ui" / "PadNames.js"
    engine = QQmlEngine()
    component = QQmlComponent(engine)
    qml = f'import QtQuick\nimport "{names_js.as_uri()}" as Names\nQtObject {{ property var out: [Names.buttonName("A", "{family}"), Names.buttonName("X", "{family}")] }}\n'
    component.setData(qml.encode(), QUrl("file:///names.qml"))
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    assert obj.property("out").toVariant() == names


def test_a_menu_row_with_a_detail_is_as_tall_as_its_text(ps5):
    from PySide6.QtCore import Q_RETURN_ARG

    _window, root = ps5
    popup = root.findChild(QObject, "popup")
    QMetaObject.invokeMethod(
        root,
        "showMenu",
        Q_ARG("QVariant", {"items": [{"label": "One line", "detail": "Short."}, {"label": "Long", "detail": "Words " * 40}]}),
        Q_ARG("QVariant", None),
    )
    until(lambda: len(value(popup, "detailHeights")) == 2)

    def height(i):
        return QMetaObject.invokeMethod(popup, "heightOf", Q_RETURN_ARG("QVariant"), Q_ARG("QVariant", i))

    row = popup.property("rowHeight")
    assert row < height(0) < 2 * row, "one line of detail, no empty band under it"
    assert height(1) > height(0) + row / 2, "three lines take their room"


def test_the_power_menu_s_way_out_asks_first_while_a_game_runs(ps5, api):
    window, root = ps5
    QMetaObject.invokeMethod(root, "launch", Q_ARG("QVariant", api.allGames.byId("dead-cells")))
    until(lambda: api.home.shown == "game")
    QMetaObject.invokeMethod(root, "askPower")
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    acts = [i["act"] for i in value(popup, "items")]
    for _ in range(len(acts) - 1):
        QTest.keyClick(window, Qt.Key.Key_Down)
    assert acts[popup.property("index")] in ("quit", "logout")
    QTest.keyClick(window, Qt.Key.Key_Return)
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True, "the game under Home would be closed")
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: dialog.property("open") is False)
    assert api.universe.currentSession, "Cancel leaves the game running"


def test_a_dialog_taller_than_the_screen_scrolls_its_text(ps5):
    window, root = ps5
    dialog = root.findChild(QObject, "dialog")
    flick = next(o for o in dialog.findChildren(QObject) if o.metaObject().className().startswith("QQuickFlickable"))
    scrolling = next(o for o in flick.findChildren(QObject) if o.metaObject().className() == "QQuickBehavior")

    def ask(spec):
        QMetaObject.invokeMethod(dialog, "show", Q_ARG("QVariant", spec), Q_ARG("QVariant", None))
        assert dialog.property("open") is True

    def close():
        QTest.keyClick(window, Qt.Key.Key_Escape)
        until(lambda: dialog.property("open") is False)

    ask({"message": "Delete the save?", "buttons": ["Cancel", "Delete"]})
    QTest.keyClick(window, Qt.Key.Key_Down)
    assert scrolling.property("targetValue") == 0 and flick.property("contentY") == 0, "a short question does not move"
    close()
    ask({"message": "A question", "detail": "A detail that goes on and on. " * 300, "buttons": ["Cancel", "OK"]})
    until(lambda: flick.property("contentHeight") > flick.property("height"))
    card = flick.parentItem()
    assert card.property("height") <= window.height(), "the card stays on the screen"
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: flick.property("contentY") > 0, "Down reads on")
    end = flick.property("contentHeight") - flick.property("height")
    # Each Down steps on from where the scroll is, so it waits for a frame of movement first.
    while scrolling.property("targetValue") < end:
        at = flick.property("contentY")
        QTest.keyClick(window, Qt.Key.Key_Down)
        until(lambda at=at: flick.property("contentY") != at)
    until(lambda: flick.property("contentY") == end, "Down reaches the end")
    close()
    ask({"message": "Again", "buttons": ["OK"]})
    until(lambda: flick.property("contentY") == 0, "a new question starts at its top")


def test_the_folder_sheet_follows_the_chip_past_the_screen_edge(ps5, tmp_path, monkeypatch):
    from universe_ui.screens import paths

    base = tmp_path / " ".join(["a folder with a long name"] * 6)
    drives = [base / f"Drive {i:02}" for i in range(30)]
    for d in drives:
        d.mkdir(parents=True)
    monkeypatch.setattr(paths, "_mounts", lambda: [str(d) for d in drives])
    window, root = ps5
    QMetaObject.invokeMethod(root, "browse", Q_ARG("QVariant", {"path": str(drives[0])}), Q_ARG("QVariant", None))
    folder = root.findChild(QObject, "folder")
    chips = until(
        lambda: next(
            (o for o in folder.findChildren(QObject) if o.metaObject().className().startswith("QQuickListView") and (o.property("count") or 0) >= 30), None
        )
    )

    def text(shown):
        return next((o for o in folder.findChildren(QObject) if o.inherits("QQuickText") and o.property("visible") and shown(str(o.property("text")))), None)

    title = until(lambda: text(lambda t: t == "Choose a folder"))
    path = until(lambda: text(lambda t: t.endswith("Drive 00")))
    right = title.mapToItem(window.contentItem(), 0, 0).x() + title.property("implicitWidth")
    until(lambda: path.mapToItem(window.contentItem(), 0, 0).x() >= right, "the whole title shows, the path beside it")
    until(lambda: path.property("truncated") is True, "a path longer than the room gives way")
    QTest.keyClick(window, Qt.Key.Key_Up)
    for _ in range(chips.property("count")):
        QTest.keyClick(window, Qt.Key.Key_Right)
    assert chips.property("currentIndex") == chips.property("count") - 1
    chip = chips.property("currentItem")
    until(lambda: chip.property("x") + chip.property("width") <= chips.property("contentX") + chips.property("width"), "the last chip is in view")
