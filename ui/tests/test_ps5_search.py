import pytest
from looks import read
from PySide6.QtCore import Qt

from conftest import until

pytestmark = pytest.mark.parametrize("look", ["ps5"], indirect=True)


def open_search(look):
    page = look.open("pages/SearchPage.qml")
    assert page.property("typing") is True
    return page


def test_a_game_found_opens_on_its_hero_at_home(look):
    page = open_search(look)
    page.setProperty("query", "dead")
    until(lambda: [g.property("id") for g in read(page, "gameHits")] == ["dead-cells"])
    look.press(Qt.Key.Key_F1)
    until(lambda: page.property("typing") is False)
    look.press(Qt.Key.Key_Return)
    home = look.find("homePage")
    until(lambda: look.root.property("depth") == 0 and home.property("zone") == "hero")
    until(lambda: (game := (read(home, "rested") or {}).get("game")) is not None and game.property("id") == "dead-cells")


def test_a_game_off_the_row_joins_it_when_opened(look):
    home = look.find("homePage")
    look.root.openGame("mirrors-edge")
    until(lambda: (game := home.property("currentGame")) is not None and game.property("id") == "mirrors-edge" and home.property("zone") == "hero")


def test_a_setting_found_lands_on_its_row_in_settings(look):
    page = open_search(look)
    look.press(Qt.Key.Key_E)
    until(lambda: page.property("tab") == 1)
    page.setProperty("query", "discrete gpu")
    until(lambda: (hits := read(page, "settingHits")) and hits[0]["target"]["page"] == "launch")
    look.press(Qt.Key.Key_F1)
    look.press(Qt.Key.Key_Return)
    settings = look.page("settingsPage")
    until(lambda: look.root.property("depth") == 1 and settings.property("sectionId") == "launch")
    until(lambda: settings.property("level") == "section" and settings.property("zone") == "rows")


def test_the_sections_are_found_before_settings_was_ever_opened(look):
    page = open_search(look)
    look.press(Qt.Key.Key_E)
    for query, section in (("themes", "themes"), ("steamgriddb", "artwork")):
        page.setProperty("query", query)
        until(lambda section=section: any(h["kind"] == "section" and h["target"]["id"] == section for h in read(page, "settingHits")), query)
