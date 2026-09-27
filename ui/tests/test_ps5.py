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
    pump(20)
    assert home.property("heroShown") is False and home.property("titleShown") is False, "the hero and the name go at once"
    moved = home.property("currentGame").property("id")
    pump(700)
    assert home.property("heroShown") is True and home.property("sideShown") is False, "the side tile comes last"
    pump(600)
    assert home.property("sideShown") is True
    assert value(home, "rested")["game"].property("id") == moved


def test_down_goes_into_the_hero_then_the_hub_and_b_goes_back_up(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(50)
    assert home.property("zone") == "hero" and home.property("scroll") > 0
    strips = [s["title"] for s in value(home, "strips")]
    assert strips[0] == "Continue where you left off" and "Captures" in strips and "About" in strips
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(50)
    assert home.property("zone") == "hub" and home.property("strip") == 0
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(50)
    assert home.property("zone") == "rail" and home.property("scroll") == 0


def test_left_of_the_first_game_is_the_welcome_hub(ps5):
    window, root = ps5
    home = root.findChild(QObject, "homePage")
    QTest.keyClick(window, Qt.Key.Key_Left)
    pump(900)
    assert value(home, "rested")["kind"] == "welcome"
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(50)
    assert home.property("zone") == "welcome"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(50)
    assert home.property("zone") == "rail"


def test_back_from_a_game_the_home_builds_itself_up_again(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    api.theme.set("ps5")
    api.theme.takeLanding()
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = root.findChild(QObject, "homePage")
    QMetaObject.invokeMethod(root, "launch", Q_ARG("QVariant", api.allGames.byId("dead-cells")))
    wait_for(fake.sessionShown, 5000)
    pump(100)
    assert api.home.shown == "game" and home.property("railReveal") == 0, "blank under the game: nothing old flashes on the way back"
    fake.stop("")
    wait_for(fake.sessionEnded, 3000)
    pump(100)
    assert home.property("railReveal") < 1 and home.property("heroShown") is False, "the row first, the hero later"
    pump(2200)
    assert home.property("railReveal") == 1 and home.property("chromeReveal") == 1 and home.property("heroShown") is True
    assert home.property("currentGame").property("id") == "dead-cells", "the game just played, focused"
    window.close()
    pump(50)
