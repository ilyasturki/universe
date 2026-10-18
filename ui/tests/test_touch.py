import pytest
from looks import Look
from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QPointF, Qt
from uitest import until

from universe_ui import gamepad


@pytest.fixture
def reprise(api):
    shown = Look(api, "reprise")
    yield shown
    shown.close()


def game_id(home):
    game = home.property("currentGame")
    return game.property("id") if game is not None else None


def tile_centre(home, i):
    def items(item):
        for child in item.childItems():
            yield child
            yield from items(child)

    rail = next(v for v in items(home) if v.metaObject().className().startswith("QQuickListView") and v.property("pitch"))
    tile = QMetaObject.invokeMethod(rail, "itemAtIndex", Q_RETURN_ARG("QQuickItem*"), Q_ARG(int, i))
    p = tile.mapToScene(QPointF(tile.width() / 2, tile.height() * 0.6))
    return p.x(), p.y()


def test_a_first_tap_picks_a_tile_a_held_one_opens_its_menu_and_the_next_tap_is_a(reprise, fake):
    home = reprise.home()
    first = until(lambda: game_id(home))
    gamepad.touch(reprise.window, [tile_centre(home, 1)])
    picked = until(lambda: game_id(home) != first and game_id(home))
    assert fake.currentSession is None, "the first tap moves the ring only"
    gamepad.touch(reprise.window, [tile_centre(home, 1)], hold_ms=800)
    until(lambda: reprise.root.property("menuOpen"), "held, the tile under the ring opens its menu as a held A does")
    reprise.press(Qt.Key.Key_Escape)
    until(lambda: not reprise.root.property("menuOpen"))
    gamepad.touch(reprise.window, [tile_centre(home, 1)])
    until(lambda: fake.currentSession, "a tap on the tile holding the ring is A: the game starts")
    assert fake.currentSession["id"] == picked


def test_a_finger_is_the_pads_glyphs_and_no_cursor(api, reprise):
    from PySide6.QtCore import QEvent
    from PySide6.QtGui import QMouseEvent

    window = reprise.window
    p = QPointF(10, 10)
    api.keys.eventFilter(window, QMouseEvent(QEvent.Type.MouseMove, p, p, p, Qt.MouseButton.NoButton, Qt.MouseButton.NoButton, Qt.KeyboardModifier.NoModifier))
    assert api.keys.mode == "mouse"
    gamepad.touch(window, [(window.width() / 2, 40)])
    until(lambda: api.keys.mode == "pad", "Qt's mouse events made from the touch leave it there")
    assert window.cursor().shape() == Qt.CursorShape.BlankCursor


def test_a_right_click_on_a_cover_opens_its_page_and_another_closes_it(reprise, fake):
    from PySide6.QtCore import QPoint
    from PySide6.QtTest import QTest

    home = reprise.home()
    root = reprise.root
    first = until(lambda: game_id(home))
    x, y = tile_centre(home, 1)
    QTest.mouseClick(reprise.window, Qt.MouseButton.RightButton, Qt.KeyboardModifier.NoModifier, QPoint(int(x), int(y)))
    until(lambda: root.property("detailOpen"))
    shown = root.property("detailGame").property("id")
    assert shown != first and game_id(home) == shown, "the ring moves to the cover clicked"
    QTest.qWait(300)
    assert root.property("detailOpen"), "the root's right click, B, does not close the page it opened"
    assert fake.currentSession is None
    QTest.mouseClick(reprise.window, Qt.MouseButton.RightButton, Qt.KeyboardModifier.NoModifier, QPoint(int(x), int(y)))
    until(lambda: not root.property("detailOpen"), "on the game page a right click is B")
