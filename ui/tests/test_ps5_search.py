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
