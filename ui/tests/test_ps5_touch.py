import pytest
from PySide6.QtCore import QObject
from PySide6.QtQuick import QQuickItem
from test_ps5 import value
from test_render import render

from conftest import until
from universe_ui import gamepad


@pytest.fixture
def deck(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, width=1280, height=800, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    until(lambda: rail_at_rest(root), "the row's tiles grow into their places at the window's size")
    yield window, root
    window.close()
    del engine


def rail_at_rest(root):
    home = root.findChild(QObject, "homePage")
    rail = next(i for i in home.findChildren(QQuickItem) if i.metaObject().className().startswith("HomeRail") and i.property("visible"))
    tiles = [t for t in rail.childItems() if t.property("kind") is not None]
    big, small, anchor = rail.property("big"), rail.property("small"), rail.property("anchorX")
    sized = all(t.property("width") == (big if t.property("focused") else small) for t in tiles)
    return sized and any(t.property("focused") and t.property("x") == anchor for t in tiles)


def tap(window, x, y, hold_ms=0):
    gamepad.touch(window, [(x, y)], hold_ms)


def swipe(window, points):
    gamepad.touch(window, points)


# 1080p units, the window's height being 800 here.
def dp(v):
    return v * 800 / 1080


def test_a_tap_on_a_tile_picks_it_and_a_second_tap_plays(deck):
    window, root = deck
    home = root.findChild(QObject, "homePage")
    assert home.property("index") == 1
    # The tile right after the focused one: its left edge sits past the large tile.
    x = dp(172 + 168 + 12 + 50)
    tap(window, x, dp(126 + 50))
    until(lambda: home.property("index") == 2, "the first tap moves the cursor")
    assert root.property("launching") is False, "the first tap moves the cursor only"
    until(lambda: rail_at_rest(root), "the picked tile slides to the focus")
    x = dp(172 + 84)
    tap(window, x, dp(126 + 84))
    until(lambda: root.property("launching") is True, "a tap on the focused tile is A: the game starts")


def test_a_sideways_swipe_on_the_row_steps_it(deck):
    window, root = deck
    home = root.findChild(QObject, "homePage")
    y = dp(126 + 50)
    swipe(window, [(dp(900), y), (dp(800), y), (dp(700), y), (dp(560), y)])
    until(lambda: home.property("index") > 1, "dragged left, the row moves on")


def test_a_swipe_up_enters_the_hero_and_the_hub(deck):
    window, root = deck
    home = root.findChild(QObject, "homePage")
    x = dp(900)
    swipe(window, [(x, dp(700)), (x, dp(620)), (x, dp(540)), (x, dp(500))])
    until(lambda: home.property("zone") == "hero")
    assert value(home, "rested")["game"].property("id") == "the-technomancer"
