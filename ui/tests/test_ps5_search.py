from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_ps5 import ps5, value  # noqa: F401  (the fixture)

from conftest import pump


def open_search(root):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SearchPage.qml"), Q_ARG("QVariant", {}))
    pump(300)
    page = root.property("topPage")
    assert page is not None and page.property("typing") is True
    return page


def test_a_game_found_opens_on_its_hero_at_home(ps5):  # noqa: F811
    window, root = ps5
    page = open_search(root)
    page.setProperty("query", "dead")
    pump(50)
    hits = [g.property("id") for g in value(page, "gameHits")]
    assert hits == ["dead-cells"]
    QTest.keyClick(window, Qt.Key.Key_F1)
    pump(50)
    assert page.property("typing") is False
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(400)
    home = root.findChild(QObject, "homePage")
    assert root.property("depth") == 0 and home.property("zone") == "hero"
    assert value(home, "rested")["game"].property("id") == "dead-cells"


def test_a_game_off_the_row_joins_it_when_opened(ps5):  # noqa: F811
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    root.openGame("mirrors-edge")
    pump(100)
    assert home.property("currentGame").property("id") == "mirrors-edge" and home.property("zone") == "hero"


def test_a_setting_found_lands_on_its_row_in_settings(ps5):  # noqa: F811
    window, root = ps5
    page = open_search(root)
    QTest.keyClick(window, Qt.Key.Key_E)
    pump(50)
    assert page.property("tab") == 1
    page.setProperty("query", "discrete gpu")
    pump(300)
    hits = value(page, "settingHits")
    assert hits and hits[0]["target"]["page"] == "launch"
    QTest.keyClick(window, Qt.Key.Key_F1)
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(500)
    top = root.property("topPage")
    assert root.property("depth") == 1 and top.property("sectionId") == "launch"
    assert top.property("level") == "section" and top.property("zone") == "rows"


def test_the_sections_are_found_before_settings_was_ever_opened(ps5):  # noqa: F811
    window, root = ps5
    page = open_search(root)
    QTest.keyClick(window, Qt.Key.Key_E)
    page.setProperty("query", "themes")
    pump(300)
    assert any(h["kind"] == "section" and h["target"]["id"] == "themes" for h in value(page, "settingHits"))
    page.setProperty("query", "steamgriddb")
    pump(300)
    assert any(h["kind"] == "section" and h["target"]["id"] == "artwork" for h in value(page, "settingHits"))


def test_settings_artwork_lists_every_game_and_fetches_the_missing_art(ps5):  # noqa: F811
    _window, root = ps5
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "artwork"}))
    pump(800)
    page = root.property("topPage")
    assert page.property("sectionId") == "artwork" and page.property("level") == "section"
    rows = value(page, "content")
    assert rows[0]["action"] == "artwork-fetch"
    assert [r for r in rows if r.get("action") == "artwork-game"], "a row per game, the ones missing art first"
