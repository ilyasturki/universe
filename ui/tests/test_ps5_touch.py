import pytest
from looks import Look, read
from PySide6.QtCore import QPointF

from conftest import until
from universe_ui import gamepad


@pytest.fixture
def deck(api):
    shown = Look(api, "ps5", height=800)
    rail = shown.find("gamesRail")
    until(lambda: at_rest(rail), "the row's tiles grow into their places at the window's size")
    yield shown, rail
    shown.close()


def tiles(rail):
    return [t for t in rail.childItems() if t.objectName() == "railTile"]


def at_rest(rail):
    big, small, anchor = rail.property("big"), rail.property("small"), rail.property("anchorX")
    shown = tiles(rail)
    sized = all(t.property("width") == (big if t.property("focused") else small) for t in shown)
    return sized and any(t.property("focused") and t.property("x") == anchor for t in shown)


def tap(window, item):
    p = item.mapToScene(QPointF(item.width() / 2, item.height() / 2))
    gamepad.touch(window, [(p.x(), p.y())])


# 1080p units, the window's height being 800 here.
def dp(v):
    return v * 800 / 1080


def test_a_tap_on_a_tile_picks_it_and_a_second_tap_plays(deck):
    shown, rail = deck
    home = shown.find("homePage")
    assert home.property("index") == 1
    after = read(home, "gameEntries")[2]["game"].property("id")
    tile = next(t for t in tiles(rail) if (game := t.property("game")) is not None and game.property("id") == after)
    tap(shown.window, tile)
    until(lambda: home.property("index") == 2, "the first tap moves the cursor")
    assert shown.root.property("launching") is False, "the first tap moves the cursor only"
    until(lambda: at_rest(rail), "the picked tile slides to the focus")
    tap(shown.window, tile)
    until(lambda: shown.root.property("launching") is True, "a tap on the focused tile is A: the game starts")


def test_a_sideways_swipe_on_the_row_steps_it(deck):
    shown, _rail = deck
    home = shown.find("homePage")
    y = dp(126 + 50)
    gamepad.touch(shown.window, [(dp(900), y), (dp(800), y), (dp(700), y), (dp(560), y)])
    until(lambda: home.property("index") > 1, "dragged left, the row moves on")


def test_a_swipe_up_enters_the_hero_and_the_hub(deck):
    shown, _rail = deck
    home = shown.find("homePage")
    x = dp(900)
    gamepad.touch(shown.window, [(x, dp(700)), (x, dp(620)), (x, dp(540)), (x, dp(500))])
    until(lambda: home.property("zone") == "hero")
    assert read(home, "rested")["game"].property("id") == "the-technomancer"
