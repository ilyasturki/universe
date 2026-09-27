import pytest
from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QPointF, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest
from test_render import render

from conftest import pump
from universe_ui import gamepad


@pytest.fixture
def deck(api):
    api.theme.set("switch2")
    api.theme.takeLanding()
    engine, window = render(api, width=1280, height=800, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root
    window.close()
    pump(50)
    del engine


def centre(item, dx=0.5, dy=0.5):
    p = item.mapToScene(QPointF(item.width() * dx, item.height() * dy))
    return p.x(), p.y()


def list_view(parent):
    return next(i for i in parent.findChildren(QQuickItem) if i.inherits("QQuickListView"))


def row_item(view, i):
    return QMetaObject.invokeMethod(view, "itemAtIndex", Q_RETURN_ARG("QQuickItem*"), Q_ARG(int, i))


# A Modal's children: the scrim, then the card (its `card` alias is a QQuickRectangle PySide cannot hand over).
def card(modal):
    return modal.childItems()[1]


def bar_icon(bar, i):
    return (bar.property("endPad") + i * bar.property("pitch") + bar.property("iconSize") / 2) / bar.width()


def tap(window, point, hold_ms=0):
    gamepad.touch(window, [point], hold_ms)
    pump(150)


def test_a_tap_on_a_home_tile_picks_it_and_the_next_one_is_a(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    tile = row_item(list_view(home), 1)
    assert home.property("index") == 0
    tap(window, centre(tile))
    assert home.property("index") == 1 and root.property("launching") is False, "the first tap moves the cursor only"
    tap(window, centre(tile))
    assert root.property("launching") is True, "a tap on the tile under the cursor is A: the game starts"


def test_a_tap_on_the_row_takes_the_focus_back_from_the_bar(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    bar = root.findChild(QQuickItem, "bottomBar")
    tap(window, centre(bar, bar_icon(bar, 0)))
    assert root.property("depth") == 1, "a bar icon is a button: one tap opens All Software"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(200)
    assert root.property("depth") == 0 and root.property("homeFocus") == "bar"
    tap(window, centre(row_item(list_view(home), 0)))
    assert root.property("homeFocus") == "home" and home.property("activeFocus"), "the finger takes the focus along, as Up does"
    assert home.property("index") == 0 and root.property("launching") is False, "the tile had the cursor but not the focus: no A"


def test_a_long_press_on_a_tile_opens_its_options(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    tap(window, centre(row_item(list_view(home), 2)), hold_ms=600)
    pump(200)
    top = root.property("topPage")
    assert home.property("index") == 2 and root.property("launching") is False
    assert root.property("depth") == 1 and top.metaObject().className().startswith("SoftwareOptionsPage"), "held, the tile is picked and gets +"


def test_a_swipe_scrolls_the_home_row_and_picks_nothing(deck):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    row = list_view(home)
    x, y = centre(row_item(row, 1))
    before = row.property("contentX")
    gamepad.touch(window, [(x + 300 - k * 60, y) for k in range(10)])
    pump(600)
    assert row.property("contentX") > before + 300, "the row follows the finger left, and the fling carries it on"
    assert home.property("index") == 0 and root.property("launching") is False and root.property("depth") == 0, "a swipe is no tap"


def test_the_power_picker_and_its_dialog_take_taps_and_keep_the_finger_off_home(deck, api):
    window, root = deck
    home = root.findChild(QQuickItem, "homePage")
    bar = root.findChild(QQuickItem, "bottomBar")
    picker = root.findChild(QQuickItem, "picker")
    dialog = root.findChild(QQuickItem, "dialog")
    row = list_view(home)
    tap(window, centre(bar, bar_icon(bar, 6)))
    assert picker.property("open") is True and picker.property("title") == "Power Options"

    x, y = centre(card(picker), 1.0, 0.5)
    before = row.property("contentX")
    gamepad.touch(window, [(x + 120 - k * 60, y) for k in range(10)])
    pump(600)
    assert row.property("contentX") == before, "a swipe over the scrim scrolls nothing under it"
    assert picker.property("open") is True, "and is no tap beside the card"
    tap(window, (x + 120, y))
    assert picker.property("open") is False and home.property("index") == 0, "a tap beside the card is B, and reaches no tile"

    tap(window, centre(bar, bar_icon(bar, 6)))
    choice = row_item(list_view(picker), picker.property("choices").toVariant().index("Turn Off"))
    tap(window, centre(choice))
    assert picker.property("open") is False
    assert dialog.property("open") is True and dialog.property("message") == "Turn off the system?", "a choice is a button: one tap picks it"
    assert dialog.property("index") == 0

    buttons = card(dialog)
    rise = 113 * 800 / 1080 / 2 / buttons.height()
    tap(window, centre(buttons, 1.5 / len(dialog.property("buttons").toVariant()), 1 - rise))
    assert dialog.property("open") is False and dialog.property("index") == 1, "a button off the cursor is pressed at once"
    for _ in range(100):
        if api.universe.core.powered:
            break
        pump(30)
    assert api.universe.core.powered == ["power_off"]


def test_a_long_picker_scrolls_under_the_finger_and_a_choice_is_one_tap(deck):
    window, root = deck
    picker = root.findChild(QQuickItem, "picker")
    spec = {"title": "Many", "choices": [f"Choice {k}" for k in range(20)]}
    QMetaObject.invokeMethod(root, "pick", Q_ARG("QVariant", spec), Q_ARG("QVariant", None))
    pump(300)
    choices = list_view(picker)
    x, y = centre(choices, 0.5, 0.8)
    gamepad.touch(window, [(x, y - k * 40) for k in range(10)])
    pump(600)
    assert choices.property("contentY") > 300, "the list follows the finger, the card's own block under it"
    assert picker.property("open") is True and picker.property("index") == 0, "a swipe picks nothing"
    x, y = centre(choices)
    local = choices.mapFromScene(QPointF(x, y))
    shown = QMetaObject.invokeMethod(
        choices, "indexAt", Q_RETURN_ARG(int), Q_ARG(float, local.x() + choices.property("contentX")), Q_ARG(float, local.y() + choices.property("contentY"))
    )
    assert shown > 3
    tap(window, (x, y))
    assert picker.property("open") is False and picker.property("index") == shown
