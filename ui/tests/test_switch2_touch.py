import pytest
from looks import Look, call, invoke
from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QPointF, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest

from conftest import until
from universe_ui import gamepad


@pytest.fixture
def deck(api):
    shown = Look(api, "switch2", height=800)
    yield shown.window, shown.root
    shown.close()


def centre(item, dx=0.5, dy=0.5):
    p = item.mapToScene(QPointF(item.width() * dx, item.height() * dy))
    return p.x(), p.y()


def row_item(view, i):
    return QMetaObject.invokeMethod(view, "itemAtIndex", Q_RETURN_ARG("QQuickItem*"), Q_ARG(int, i))


def shown(modal):
    return modal.property("open") is True and modal.findChild(QQuickItem, "modalCard").property("opacity") == 1


def bar_icon(bar, i):
    return (bar.property("endPad") + i * bar.property("pitch") + bar.property("iconSize") / 2) / bar.width()


def tap(window, point, hold_ms=0):
    gamepad.touch(window, [point], hold_ms)


def test_a_tap_on_a_home_tile_picks_it_and_the_next_one_is_a(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    row = home.findChild(QQuickItem, "homeRow")
    tile = row_item(row, 1)
    assert home.property("index") == 0
    tap(window, centre(tile))
    until(lambda: home.property("index") == 1, "the first tap moves the cursor only")
    assert root.property("launching") is False, "the first tap moves the cursor only"
    tap(window, centre(tile))
    until(lambda: root.property("launching") is True, "a tap on the tile under the cursor is A: the game starts")


def test_a_tap_on_the_row_takes_the_focus_back_from_the_bar(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    bar = root.findChild(QQuickItem, "bottomBar")
    tap(window, centre(bar, bar_icon(bar, 0)))
    until(lambda: root.property("depth") == 1, "a bar icon is a button: one tap opens All Software")
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == 0 and root.property("homeFocus") == "bar")
    row = home.findChild(QQuickItem, "homeRow")
    tap(window, centre(row_item(row, 0)))
    until(lambda: root.property("homeFocus") == "home" and home.property("activeFocus"), "the finger takes the focus along, as Up does")
    assert home.property("index") == 0 and root.property("launching") is False, "the tile had the cursor but not the focus: no A"


def test_a_long_press_on_a_tile_opens_its_options(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    row = home.findChild(QQuickItem, "homeRow")
    tap(window, centre(row_item(row, 2)), hold_ms=600)
    until(lambda: root.property("depth") == 1, "held, the tile is picked and gets +")
    assert home.property("index") == 2 and root.property("launching") is False
    assert root.property("topPage").objectName() == "softwareOptionsPage", "held, the tile is picked and gets +"


def test_a_swipe_scrolls_the_home_row_and_picks_nothing(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    row = home.findChild(QQuickItem, "homeRow")
    x, y = centre(row_item(row, 1))
    before = row.property("contentX")
    gamepad.touch(window, [(x + 300 - k * 60, y) for k in range(10)])
    until(lambda: row.property("contentX") > before + 300, "the row follows the finger left, and the fling carries it on")
    assert home.property("index") == 0 and root.property("launching") is False and root.property("depth") == 0, "a swipe is no tap"


def test_the_power_picker_and_its_dialog_take_taps_and_keep_the_finger_off_home(deck, api):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    bar = root.findChild(QQuickItem, "bottomBar")
    picker = root.findChild(QQuickItem, "picker")
    dialog = root.findChild(QQuickItem, "dialog")
    row = home.findChild(QQuickItem, "homeRow")
    tap(window, centre(bar, bar_icon(bar, 6)))
    until(lambda: shown(picker))

    x, y = centre(picker.findChild(QQuickItem, "modalCard"), 1.0, 0.5)
    before = row.property("contentX")
    gamepad.touch(window, [(x + 120 - k * 60, y) for k in range(10)])
    until(lambda: not row.property("moving"))
    assert row.property("contentX") == before, "a swipe over the scrim scrolls nothing under it"
    assert picker.property("open") is True, "and is no tap beside the card"
    tap(window, (x + 120, y))
    until(lambda: picker.property("open") is False, "a tap beside the card is B")
    assert home.property("index") == 0, "a tap beside the card is B, and reaches no tile"
    until(lambda: not picker.property("visible"))

    tap(window, centre(bar, bar_icon(bar, 6)))
    until(lambda: shown(picker))
    acts = [i.get("action", i.get("act")) for i in call(root, "powerItems")]
    choices = picker.findChild(QQuickItem, "choices")
    tap(window, centre(row_item(choices, acts.index("power_off"))))
    until(lambda: shown(dialog), "a choice is a button: one tap picks it")
    assert picker.property("open") is False and dialog.property("index") == 0

    buttons = dialog.findChild(QQuickItem, "modalCard")
    rise = 113 * 800 / 1080 / 2 / buttons.height()
    tap(window, centre(buttons, 1.5 / len(dialog.property("buttons").toVariant()), 1 - rise))
    until(lambda: dialog.property("open") is False, "a button off the cursor is pressed at once")
    assert dialog.property("index") == 1, "a button off the cursor is pressed at once"
    assert until(lambda: api.universe.core.powered) == ["power_off"]


def test_a_long_picker_scrolls_under_the_finger_and_a_choice_is_one_tap(deck):
    window, root = deck
    picker = root.findChild(QQuickItem, "picker")
    invoke(root, "pick", {"title": "Many", "choices": [f"Choice {k}" for k in range(20)]}, None)
    until(lambda: shown(picker))
    choices = picker.findChild(QQuickItem, "choices")
    x, y = centre(choices, 0.5, 0.8)
    gamepad.touch(window, [(x, y - k * 40) for k in range(10)])
    until(lambda: choices.property("contentY") > 300, "the list follows the finger, the card's own block under it")
    until(lambda: not choices.property("moving"))
    assert picker.property("open") is True and picker.property("index") == 0, "a swipe picks nothing"
    x, y = centre(choices)
    local = choices.mapFromScene(QPointF(x, y))
    landed = QMetaObject.invokeMethod(
        choices, "indexAt", Q_RETURN_ARG(int), Q_ARG(float, local.x() + choices.property("contentX")), Q_ARG(float, local.y() + choices.property("contentY"))
    )
    assert landed > 3
    tap(window, (x, y))
    until(lambda: picker.property("open") is False)
    assert picker.property("index") == landed
