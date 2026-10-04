import os

from PySide6.QtCore import QObject, Qt
from PySide6.QtTest import QTest
from test_render import lit_fraction, render, settle

from conftest import until
from universe_ui import host


def key(window, k):
    QTest.keyClick(window, k)


def covered_fraction(image, rows):
    small = image.scaled(96, 54)
    covered = sum(1 for y in rows(small.height()) for x in range(small.width()) if small.pixelColor(x, y).alpha() > 10)
    return covered / (small.width() * len(rows(small.height())))


# A view's delegates hang off its content item, out of reach of findChildren.
def items_named(item, name):
    found = [item] if item.objectName() == name else []
    for child in item.childItems():
        found += items_named(child, name)
    return found


def start(api, fake, ident):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api)
    overlay = host.create_overlay(engine, window.size())
    assert overlay is not None
    api.home.attachOverlay(overlay)
    overlay.show()
    settle(overlay)
    fake.launch(ident, "")
    until(lambda: api.home.shown == "game")
    return engine, window, overlay


def open_cc(api, overlay):
    api.home.openDock()
    overlay.requestActivate()
    until(overlay.isActive)
    cc = overlay.property("contentItem").childItems()[0].property("item")
    until(lambda: cc.property("open") is True)
    return cc


def shot(overlay, name):
    where = os.environ.get("UNIVERSE_TEST_SHOTS")
    if where:
        overlay.grabWindow().save(os.path.join(where, name + ".png"))


def stop(api, window, overlay):
    api.home.stop()
    until(lambda: api.universe.currentSession is None)
    window.close()
    overlay.close()


def test_the_control_center_lays_its_cards_over_the_bottom_of_the_game(api, fake):
    _engine, window, overlay = start(api, fake, "batman-arkham-origins")
    cc = open_cc(api, overlay)
    assert cc.property("zone") == "cards"
    until(lambda: cc.appearOf(0) > 0)
    shot(overlay, "cc-opening")
    assert cc.appearOf(0) == cc.appearOf(1) and cc.appearOf(3) < 1, "the first two cards land together, the rest follow"
    until(lambda: all(cc.appearOf(i) == 1 for i in range(len(cc.property("cards").toVariant()))))
    ids = [c["id"] for c in cc.property("cards").toVariant()]
    assert ids[:2] == ["game", "hub"] and "trophies" in ids and "captures" in ids
    shot(overlay, "cc-open")
    image = overlay.grabWindow()
    assert covered_fraction(image, lambda h: range(h - 12, h)) > 0.9, "the lower band is drawn over the game"
    assert image.pixelColor(4, 4).alpha() == 0, "the top of the frame stays clear"
    assert lit_fraction(image, "#000000") > 0.01
    stop(api, window, overlay)


def test_the_power_panel_logs_out_in_the_universe_session(universe_session, api, fake):
    _engine, window, overlay = start(api, fake, "mirrors-edge")
    cc = open_cc(api, overlay)
    icons = [i["id"] for i in cc.property("icons").toVariant()]
    cc.setProperty("icon", icons.index("power"))
    rows = [r["id"] for r in cc.property("panelRows").toVariant()]
    assert ("logout" in rows) is universe_session and rows[0] == "quit", rows
    stop(api, window, overlay)


def test_the_bar_opens_a_panel_over_its_icon_and_b_steps_back_out(api, fake):
    _engine, window, overlay = start(api, fake, "mirrors-edge")
    cc = open_cc(api, overlay)
    key(overlay, Qt.Key.Key_Down)
    assert cc.property("zone") == "bar"
    key(overlay, Qt.Key.Key_Right)
    assert cc.property("current").toVariant()["id"] == "game"
    key(overlay, Qt.Key.Key_Return)
    until(lambda: cc.property("zone") == "panel")
    cards = cc.findChild(QObject, "ccCards")
    until(lambda: cards.property("opacity") < 0.5, "the cards step back behind the panel")
    shot(overlay, "cc-panel")
    assert [r["id"] for r in cc.property("panelRows").toVariant()] == ["resume", "details", "pause", "quit"]
    was = api.home.pauseOnHome
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Return)
    assert api.home.pauseOnHome is (not was)
    key(overlay, Qt.Key.Key_Escape)
    assert cc.property("zone") == "bar"
    key(overlay, Qt.Key.Key_Escape)
    until(lambda: api.home.open is False, "B closes the Control Center")
    stop(api, window, overlay)


def test_the_game_hub_card_lands_on_the_games_hero_at_home(api, fake):
    _engine, window, overlay = start(api, fake, "dead-cells")
    cc = open_cc(api, overlay)
    key(overlay, Qt.Key.Key_Right)
    assert cc.property("cards").toVariant()[cc.property("card")]["id"] == "hub"
    key(overlay, Qt.Key.Key_Return)
    until(lambda: api.home.shown == "launcher" and api.home.open is False)
    root = window.property("contentItem").childItems()[0].property("item")
    home = root.findChild(QObject, "homePage")
    until(lambda: home.property("zone") == "hero" and (game := home.property("currentGame")) is not None and game.property("id") == "dead-cells")
    assert api.home.takeLanding() == "", "taken once"
    api.home.toGame()
    until(lambda: api.home.shown == "game")
    stop(api, window, overlay)


def test_the_trophies_card_grows_into_the_games_list(api, fake):
    _engine, window, overlay = start(api, fake, "batman-arkham-origins")
    cc = open_cc(api, overlay)
    ids = [c["id"] for c in cc.property("cards").toVariant()]
    for _ in range(ids.index("trophies")):
        key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Return)
    until(lambda: cc.property("zone") == "sheet" and cc.property("sheet") == "trophies")
    until(lambda: len(api.screens.dockAchievements.rows) > 0)
    rows = api.screens.dockAchievements.rows
    earned = until(lambda: (found := items_named(cc, "trophyEarned")) and len(found) >= min(len(rows), 3) and found)
    assert any(not r["unlocked"] for r in rows) and any(r["unlocked"] for r in rows)
    shown = [label.property("visible") for label in earned]
    assert any(shown) and not all(shown), "a locked trophy says nothing where an earned one has its date"
    assert not any(label.property("text").endswith("of players") for label in items_named(cc, "trophyRarity")), "the trophy page's wording"
    shot(overlay, "cc-trophies")
    key(overlay, Qt.Key.Key_Escape)
    assert cc.property("zone") == "cards"
    stop(api, window, overlay)
