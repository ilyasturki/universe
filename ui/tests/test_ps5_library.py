import pytest
from looks import invoke, read
from PySide6.QtCore import Qt

from conftest import until

pytestmark = pytest.mark.parametrize("look", ["ps5"], indirect=True)


@pytest.fixture
def library(look):
    return look.open("pages/LibraryPage.qml")


def titles(page):
    return [g.property("title") for g in read(page, "items")]


def test_the_collection_holds_the_whole_library_and_a_search_or_a_filter_narrows_it(api, library):
    assert len(titles(library)) == api.allGames.count
    library.setProperty("query", "batman")
    until(lambda: titles(library) and all("batman" in t.casefold() for t in titles(library)))
    library.setProperty("query", "")
    until(lambda: len(titles(library)) == api.allGames.count)
    library.setProperty("source", "gog")
    until(lambda: read(library, "items") and all(g.property("source") == "gog" for g in read(library, "items")))
    assert library.property("filtering") is True
    invoke(library, "drawerAction", "reset")
    until(lambda: library.property("filtering") is False and len(titles(library)) == api.allGames.count)


def test_a_sort_mode_reorders_the_grid(library):
    library.setProperty("sortMode", 1)
    until(lambda: titles(library) == sorted(titles(library), key=str.casefold))
    by_name = titles(library)
    library.setProperty("sortMode", 2)
    until(lambda: titles(library) == sorted(by_name, key=str.casefold, reverse=True))


def test_the_gamelists_tab_lists_them_favourites_first_and_opens_one(look, library):
    look.press(Qt.Key.Key_E, 2)
    until(lambda: library.property("tab") == 2 and library.property("showingLists") is True)
    lists = read(library, "items")
    assert lists[0]["key"] == "favourites" and len(lists) > 1
    look.press(Qt.Key.Key_Return)
    until(lambda: library.property("openList") == "favourites" and library.property("showingLists") is False)
    assert all(g.property("favorite") for g in read(library, "items"))
    look.press(Qt.Key.Key_Escape)
    until(lambda: library.property("openList") == "" and library.property("showingLists") is True)


def test_start_on_a_game_opens_its_menu(look, library):
    assert library.property("zone") == "grid"
    look.press(Qt.Key.Key_F1)
    until(lambda: look.root.property("modal") is True, "Options brings the game's menu up")
    assert {"info", "settings", "remove"} <= {i["act"] for i in read(look.menu(), "items")}
    look.press(Qt.Key.Key_Escape)
    until(lambda: look.root.property("modal") is False)


def test_left_of_the_grid_is_the_rail_with_its_drawer_and_up_the_tabs(look, library):
    look.press(Qt.Key.Key_Left)
    until(lambda: library.property("zone") == "rail")
    look.press(Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    until(lambda: library.property("zone") == "drawer", "the rail's sort and filter opens the drawer")
    look.press(Qt.Key.Key_Escape)
    until(lambda: library.property("zone") == "rail")
    look.press(Qt.Key.Key_Right)
    until(lambda: library.property("zone") == "grid")
    look.press(Qt.Key.Key_Up)
    until(lambda: library.property("zone") == "tabs")
    look.press(Qt.Key.Key_Right)
    until(lambda: library.property("tab") == 1)
    look.press(Qt.Key.Key_Down)
    until(lambda: library.property("zone") == "grid" and library.property("tab") == 1)


def test_add_game_lists_a_file_the_stores_and_lutris(look):
    page = look.open("pages/AddGamePage.qml")
    assert {"pick_file", "store", "lutris"} <= {r.get("key") for part in read(page, "parts") for r in part["rows"]}
    assert page.property("strip") is True
