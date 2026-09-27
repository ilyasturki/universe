import pytest
from PySide6.QtCore import QObject
from test_ps5 import value
from test_render import render

from conftest import pump
from universe_ui import gamepad


@pytest.fixture
def deck(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, width=1280, height=800, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root
    window.close()
    pump(50)
    del engine


def tap(window, x, y, hold_ms=0):
    gamepad.touch(window, [(x, y)], hold_ms)
    pump(150)


def swipe(window, points):
    gamepad.touch(window, points)
    pump(150)


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
    assert home.property("index") == 2 and root.property("launching") is False, "the first tap moves the cursor only"
    pump(900)
    x = dp(172 + 84)
    tap(window, x, dp(126 + 84))
    assert root.property("launching") is True, "a tap on the focused tile is A: the game starts"


def test_a_sideways_swipe_on_the_row_steps_it(deck):
    window, _root = deck
    home = _root.findChild(QObject, "homePage")
    y = dp(126 + 50)
    swipe(window, [(dp(900), y), (dp(800), y), (dp(700), y), (dp(560), y)])
    assert home.property("index") > 1, "dragged left, the row moves on"


def test_a_swipe_up_enters_the_hero_and_the_hub(deck):
    window, root = deck
    home = root.findChild(QObject, "homePage")
    x = dp(900)
    swipe(window, [(x, dp(700)), (x, dp(620)), (x, dp(540)), (x, dp(500))])
    assert home.property("zone") == "hero"
    assert value(home, "rested")["game"].property("id") == "the-technomancer"
