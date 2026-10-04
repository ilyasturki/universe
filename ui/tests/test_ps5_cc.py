import os

import pytest
from looks import Look, read, settle
from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QObject, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest

from conftest import until
from universe_ui import host


@pytest.fixture
def ps5(api):
    shown = Look(api, "ps5", activate=False)
    overlay = host.create_overlay(shown.engine, shown.window.size())
    api.home.attachOverlay(overlay)
    overlay.show()
    settle(overlay)
    yield shown, overlay
    api.home.stop()
    until(lambda: api.universe.currentSession is None)
    overlay.close()
    shown.close()


def open_cc(api, fake, overlay, game):
    fake.launch(game, "")
    until(lambda: api.home.shown == "game")
    api.home.openDock()
    overlay.requestActivate()
    until(overlay.isActive)
    cc = overlay.findChild(QObject, "controlCenter")
    until(lambda: cc.property("open") is True)
    return cc


def key(overlay, k):
    QTest.keyClick(overlay, k)


def covered_fraction(image, rows):
    small = image.scaled(96, 54)
    covered = sum(1 for y in rows(small.height()) for x in range(small.width()) if small.pixelColor(x, y).alpha() > 10)
    return covered / (small.width() * len(rows(small.height())))


def shot(overlay, name):
    if where := os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(os.path.join(where, name + ".png"))


def test_the_control_center_lays_its_cards_over_the_bottom_of_the_game(api, fake, ps5):
    _shown, overlay = ps5
    cc = open_cc(api, fake, overlay, "batman-arkham-origins")
    assert cc.property("zone") == "cards"
    ids = [c["id"] for c in read(cc, "cards")]
    assert ids[:2] == ["game", "hub"] and "trophies" in ids and "captures" in ids
    until(lambda: cc.appearOf(0) > 0)
    assert cc.appearOf(0) == cc.appearOf(1) and cc.appearOf(3) < 1, "the first two cards land together, the rest follow"
    until(lambda: cc.property("dimIn") == 1)
    shot(overlay, "cc-open")
    image = overlay.grabWindow()
    assert covered_fraction(image, lambda h: range(h - 12, h)) > 0.9, "the lower band is drawn over the game"
    assert image.pixelColor(4, 4).alpha() == 0, "the top of the frame stays clear"


def test_the_power_panel_logs_out_in_the_universe_session(universe_session, api, fake, ps5):
    _shown, overlay = ps5
    cc = open_cc(api, fake, overlay, "mirrors-edge")
    cc.setProperty("icon", [i["id"] for i in read(cc, "icons")].index("power"))
    rows = [r["id"] for r in read(cc, "panelRows")]
    assert ("logout" in rows) is universe_session and rows[0] == "quit", rows


def test_the_bar_opens_a_panel_over_its_icon_and_b_steps_back_out(api, fake, ps5):
    _shown, overlay = ps5
    cc = open_cc(api, fake, overlay, "mirrors-edge")
    key(overlay, Qt.Key.Key_Down)
    assert cc.property("zone") == "bar"
    key(overlay, Qt.Key.Key_Right)
    assert read(cc, "current")["id"] == "game"
    key(overlay, Qt.Key.Key_Return)
    until(lambda: cc.property("zone") == "panel")
    cards = cc.findChild(QObject, "ccCards")
    until(lambda: cards.property("opacity") < 0.5, "the cards step back behind the panel")
    shot(overlay, "cc-panel")
    assert [r["id"] for r in read(cc, "panelRows")] == ["resume", "details", "pause", "quit"]
    was = api.home.pauseOnHome
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Return)
    assert api.home.pauseOnHome is (not was)
    key(overlay, Qt.Key.Key_Escape)
    assert cc.property("zone") == "bar"
    key(overlay, Qt.Key.Key_Escape)
    until(lambda: api.home.open is False, "B closes the Control Center")


def test_the_game_hub_card_lands_on_the_games_hero_at_home(api, fake, ps5):
    shown, overlay = ps5
    cc = open_cc(api, fake, overlay, "dead-cells")
    key(overlay, Qt.Key.Key_Right)
    assert read(cc, "cards")[cc.property("card")]["id"] == "hub"
    key(overlay, Qt.Key.Key_Return)
    until(lambda: api.home.shown == "launcher" and api.home.open is False)
    home = shown.find("homePage")
    until(lambda: home.property("zone") == "hero" and (game := home.property("currentGame")) is not None and game.property("id") == "dead-cells")
    assert api.home.takeLanding() == "", "taken once"
    api.home.toGame()
    until(lambda: api.home.shown == "game")


def test_the_trophies_card_grows_into_the_games_list(api, fake, ps5):
    _shown, overlay = ps5
    cc = open_cc(api, fake, overlay, "batman-arkham-origins")
    ids = [c["id"] for c in read(cc, "cards")]
    for _ in range(ids.index("trophies")):
        key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Return)
    until(lambda: cc.property("zone") == "sheet" and cc.property("sheet") == "trophies")
    rows = until(lambda: api.screens.dockAchievements.rows)
    assert any(not r["unlocked"] for r in rows) and any(r["unlocked"] for r in rows)
    trophies = cc.findChild(QQuickItem, "ccTrophies")

    def dated():
        shown = [QMetaObject.invokeMethod(trophies, "itemAtIndex", Q_RETURN_ARG("QQuickItem*"), Q_ARG(int, i)) for i in range(len(rows))]
        return [row.findChild(QObject, "trophyEarned").property("text") != "" for row in shown if row]

    until(lambda: any(dated()) and not all(dated()), "a locked trophy says nothing where an earned one has its date")
    shot(overlay, "cc-trophies")
    key(overlay, Qt.Key.Key_Escape)
    assert cc.property("zone") == "cards"
