import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
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


def test_left_of_the_first_game_is_the_welcome_hub(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    QTest.keyClick(window, Qt.Key.Key_Left)
    until(lambda: value(home, "rested")["kind"] == "welcome")
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: home.property("zone") == "welcome")
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: home.property("zone") == "rail")


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
