from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_ps5 import ps5, value  # noqa: F401  (the fixture)

from conftest import until


def open_search(root):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SearchPage.qml"), Q_ARG("QVariant", {}))
    page = until(lambda: (top := root.property("topPage")) is not None and top.property("activeFocus") and top)
    assert page.property("typing") is True
    return page


def test_a_game_found_opens_on_its_hero_at_home(ps5):  # noqa: F811
    window, root = ps5
    page = open_search(root)
    page.setProperty("query", "dead")
    until(lambda: [g.property("id") for g in value(page, "gameHits")] == ["dead-cells"])
    QTest.keyClick(window, Qt.Key.Key_F1)
    until(lambda: page.property("typing") is False)
    QTest.keyClick(window, Qt.Key.Key_Return)
    home = root.findChild(QObject, "homePage")
    until(lambda: root.property("depth") == 0 and home.property("zone") == "hero")
    until(lambda: (game := (value(home, "rested") or {}).get("game")) is not None and game.property("id") == "dead-cells")


def test_a_game_off_the_row_joins_it_when_opened(ps5):  # noqa: F811
    _window, root = ps5
    home = root.findChild(QObject, "homePage")
    root.openGame("mirrors-edge")
    until(lambda: (game := home.property("currentGame")) is not None and game.property("id") == "mirrors-edge" and home.property("zone") == "hero")


def test_a_setting_found_lands_on_its_row_in_settings(ps5):  # noqa: F811
    window, root = ps5
    page = open_search(root)
    QTest.keyClick(window, Qt.Key.Key_E)
    until(lambda: page.property("tab") == 1)
    page.setProperty("query", "discrete gpu")
    until(lambda: (hits := value(page, "settingHits")) and hits[0]["target"]["page"] == "launch")
    QTest.keyClick(window, Qt.Key.Key_F1)
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 1 and root.property("topPage").property("sectionId") == "launch")
    top = root.property("topPage")
    until(lambda: top.property("level") == "section" and top.property("zone") == "rows")


def test_the_sections_are_found_before_settings_was_ever_opened(ps5):  # noqa: F811
    window, root = ps5
    page = open_search(root)
    QTest.keyClick(window, Qt.Key.Key_E)
    page.setProperty("query", "themes")
    until(lambda: any(h["kind"] == "section" and h["target"]["id"] == "themes" for h in value(page, "settingHits")))
    page.setProperty("query", "steamgriddb")
    until(lambda: any(h["kind"] == "section" and h["target"]["id"] == "artwork" for h in value(page, "settingHits")))


def test_settings_artwork_lists_every_game_and_fetches_the_missing_art(ps5):  # noqa: F811
    _window, root = ps5
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "artwork"}))
    page = root.property("topPage")
    until(lambda: page.property("sectionId") == "artwork" and page.property("level") == "section")
    rows = until(lambda: value(page, "content"))
    assert rows[0]["action"] == "artwork-fetch"
    until(lambda: [r for r in value(page, "content") if r.get("action") == "artwork-game"], "a row per game, the ones missing art first")
