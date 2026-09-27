import pytest
from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot
from PySide6.QtQuick import QQuickView

from conftest import pump
from universe_ui import gamepad, host

SCENE = """
import QtQuick
import "UI_DIR"

Rectangle {
    id: root
    width: 800
    height: 600
    property alias current: tile.current
    property int picks: 0

    Flickable {
        id: view
        objectName: "view"
        anchors.fill: parent
        contentHeight: 3000
        interactive: false

        Rectangle {
            id: tile
            property bool current: false
            x: 100; y: 100; width: 200; height: 200
            Pointer {
                current: tile.current
                onPicked: { root.picks++; tile.current = true; }
            }
        }
        Wheel {}
    }
}
"""


class Keys(QObject):
    modeChanged = Signal()

    def __init__(self):
        super().__init__()
        self.calls = []

    @Slot(str)
    def hold(self, action):
        self.calls.append(("hold", action))

    @Slot(str)
    def release(self, action):
        self.calls.append(("release", action))

    @Slot(str)
    def press(self, action):
        self.calls.append(("press", action))

    mode = Property(str, lambda self: "pad", notify=modeChanged)


class Api(QObject):
    def __init__(self):
        super().__init__()
        self._keys = Keys()

    keys = Property(QObject, lambda self: self._keys, constant=True)


@pytest.fixture
def scene(app, tmp_path):
    path = tmp_path / "scene.qml"
    path.write_text(SCENE.replace("UI_DIR", (host.QML_DIR / "ui").as_uri()))
    api = Api()
    view = QQuickView()
    view.rootContext().setContextProperty("api", api)
    view.setSource(QUrl.fromLocalFile(str(path)))
    assert view.status() == QQuickView.Status.Ready, view.errors()
    view.show()
    pump(50)
    yield view, api.keys
    view.close()
    view.deleteLater()
    pump(20)


def test_a_first_tap_picks_and_the_next_one_is_a(scene):
    view, keys = scene
    root = view.rootObject()
    gamepad.touch(view, [(200, 200)])
    pump(50)
    assert root.property("picks") == 1 and keys.calls == [], "the first tap moves the ring only"
    gamepad.touch(view, [(200, 200)])
    pump(50)
    assert keys.calls == [("press", "Accept")], "a tap on the item holding the ring is A, once, at the release"


def test_a_long_press_holds_a_until_the_finger_lifts(scene):
    view, keys = scene
    view.rootObject().setProperty("current", True)
    gamepad.touch(view, [(200, 200)], hold_ms=400)
    pump(50)
    assert keys.calls == [("hold", "Accept"), ("release", "Accept")], "held as a held A opens the game menu"


def test_a_swipe_scrolls_the_view_and_is_no_tap(scene):
    view, keys = scene
    root = view.rootObject()
    root.setProperty("current", True)
    flick = root.findChild(QObject, "view")
    gamepad.touch(view, [(200, 500 - k * 40) for k in range(10)])
    pump(600)
    assert flick.property("contentY") > 300, "the content follows the finger up, and the fling carries it on"
    assert keys.calls == [] and root.property("picks") == 0, "a finger that lands to scroll presses nothing"


def test_a_finger_is_the_pads_glyphs_and_no_cursor(api, scene):
    from PySide6.QtCore import QEvent, QPointF, Qt
    from PySide6.QtGui import QMouseEvent

    view, _ = scene
    api.keys.watch(view)
    p = QPointF(10, 10)
    api.keys.eventFilter(view, QMouseEvent(QEvent.Type.MouseMove, p, p, p, Qt.MouseButton.NoButton, Qt.MouseButton.NoButton, Qt.KeyboardModifier.NoModifier))
    assert api.keys.mode == "mouse"
    gamepad.touch(view, [(500, 500)])
    pump(20)
    assert api.keys.mode == "pad", "Qt's mouse events made from the touch leave it there"
    assert view.cursor().shape() == Qt.CursorShape.BlankCursor
