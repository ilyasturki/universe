import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest
from test_render import render, settle

from conftest import until


@pytest.fixture
def library(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/LibraryPage.qml"), Q_ARG("QVariant", {}))
    settle(window)
    page = root.property("topPage")
    assert page is not None and page.property("tabNames") is not None
    yield window, root, page
    window.close()
    del engine


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def titles(page):
    return [g.property("title") for g in value(page, "items")]


def grid_of(page):
    return next(i for i in page.findChildren(QQuickItem) if i.property("lists") is not None and i.property("tabNames") is not None)


def test_the_collection_holds_the_whole_library(api, library):
    _window, _root, page = library
    assert len(value(page, "items")) == api.allGames.count
    assert page.property("countText") == f"All: {api.allGames.count}"
    assert page.property("sortText").startswith("Sort by: Most Recent")


def test_a_sort_mode_reorders_the_grid(library):
    _window, _root, page = library
    page.setProperty("sortMode", 1)
    until(lambda: titles(page) == sorted(titles(page), key=str.casefold))
    by_name = titles(page)
    page.setProperty("sortMode", 2)
    until(lambda: titles(page) == sorted(by_name, key=str.casefold, reverse=True))
    assert page.property("sortText").startswith("Sort by: Name (Z - A)")


def test_a_search_narrows_the_grid(library):
    _window, _root, page = library
    page.setProperty("query", "batman")
    until(lambda: titles(page) and all("batman" in t.casefold() for t in titles(page)))
    page.setProperty("query", "")
    until(lambda: len(titles(page)) > 2)


def test_a_filter_narrows_and_reset_clears_it(api, library):
    _window, _root, page = library
    page.setProperty("source", "gog")
    until(lambda: value(page, "items") and all(g.property("source") == "gog" for g in value(page, "items")))
    assert page.property("filtering") is True
    QMetaObject.invokeMethod(page, "drawerAction", Q_ARG("QVariant", "reset"))
    until(lambda: page.property("filtering") is False and len(value(page, "items")) == api.allGames.count)


def test_the_gamelists_tab_lists_them_and_opens_one(library):
    window, _root, page = library
    QTest.keyClick(window, Qt.Key.Key_E)
    QTest.keyClick(window, Qt.Key.Key_E)
    until(lambda: page.property("tab") == 2 and page.property("showingLists") is True)
    lists = value(page, "items")
    names = [entry["name"] for entry in lists]
    assert names[0] == "Favourites" and len(names) > 1
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: page.property("openList") == "favourites" and page.property("showingLists") is False)
    assert all(g.property("favorite") for g in value(page, "items"))
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: page.property("openList") == "" and page.property("showingLists") is True)


def test_start_on_a_game_opens_its_menu(library):
    window, root, page = library
    assert grid_of(page).property("activeFocus") is True
    QTest.keyClick(window, Qt.Key.Key_F1)
    until(lambda: root.property("modal") is True, "Options brings the game's menu up")
    popup = root.findChild(QObject, "popup")
    labels = [i["label"] for i in value(popup, "items")]
    assert "Information" in labels and "Game Settings" in labels and "Remove from Library…" in labels
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: root.property("modal") is False)


def test_left_of_the_grid_is_the_rail_and_sort_and_filter_opens_its_drawer(library):
    window, _root, page = library
    QTest.keyClick(window, Qt.Key.Key_Left)
    until(lambda: page.property("zone") == "rail")
    QTest.keyClick(window, Qt.Key.Key_Down)
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: page.property("zone") == "drawer")
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: page.property("zone") == "rail")
    QTest.keyClick(window, Qt.Key.Key_Right)
    until(lambda: page.property("zone") == "grid")


def test_up_from_the_first_row_reaches_the_tabs(library):
    window, _root, page = library
    QTest.keyClick(window, Qt.Key.Key_Up)
    until(lambda: page.property("zone") == "tabs")
    QTest.keyClick(window, Qt.Key.Key_Right)
    until(lambda: page.property("tab") == 1)
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: page.property("zone") == "grid" and page.property("tab") == 1)


def test_add_game_lists_a_file_the_stores_and_lutris(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/AddGamePage.qml"), Q_ARG("QVariant", {}))
    settle(window)
    page = root.property("topPage")
    parts = [p["label"] for p in value(page, "parts")]
    assert parts[0] == "Game File" and "Lutris" in parts
    assert page.property("strip") is True
    window.close()
