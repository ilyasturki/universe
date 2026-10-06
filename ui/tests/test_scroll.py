import pytest
from looks import invoke
from PySide6.QtCore import Property, QCoreApplication, QEvent, QObject, QUrl, Signal
from PySide6.QtQuick import QQuickView
from uitest import pump, until

from universe_ui import host

LIST = """
import QtQuick
import "UI_DIR"

Item {
    width: 400
    height: 600
    property alias index: list.currentIndex
    property alias rows: list.model

    ListView {
        id: list
        objectName: "list"
        anchors.fill: parent
        model: 60
        interactive: false
        highlightFollowsCurrentItem: false
        delegate: Item {
            width: 400
            height: 100
        }

        Scroller {}
    }
}
"""

SCRUB = """
import QtQuick
import "UI_DIR"

Item {
    property alias pos: scrub.scrubPos

    QtObject {
        id: player
        property real position: 0
        property int playbackState: 0
    }

    Scrubber {
        id: scrub
        objectName: "scrub"
        player: player
        duration: 3600000
        active: true
    }
}
"""


class Pad(QObject):
    changed = Signal()

    def __init__(self):
        super().__init__()
        self._right_x = 0.0

    def set_right_x(self, value):
        self._right_x = value
        self.changed.emit()

    rightX = Property(float, lambda self: self._right_x, notify=changed)


class Api(QObject):
    def __init__(self):
        super().__init__()
        self._pad = Pad()

    pad = Property(QObject, lambda self: self._pad, constant=True)


@pytest.fixture
def load(app, tmp_path):
    views = []

    def load(scene):
        path = tmp_path / f"scene{len(views)}.qml"
        path.write_text(scene.replace("UI_DIR", (host.QML_DIR / "ui").as_uri()))
        api = Api()
        view = QQuickView()
        view.rootContext().setContextProperty("api", api)
        view.setSource(QUrl.fromLocalFile(str(path)))
        assert view.status() == QQuickView.Status.Ready, view.errors()
        view.show()
        until(view.isExposed)
        views.append((view, api))
        return view.rootObject(), api

    yield load
    for view, _ in views:
        view.close()
        view.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.Type.DeferredDelete)


def shown(root, index):
    """Where row `index` sits in the view: 0 at its top edge, 500 at its bottom one."""
    return index * 100 - root.findChild(QObject, "list").property("contentY")


def test_a_row_past_the_edge_slides_in_and_rests_on_it(load):
    root, _ = load(LIST)
    root.setProperty("index", 8)
    pump(40)
    assert 0 < shown(root, 8) - 500 < 199, "on its way, not there at once"
    until(lambda: abs(shown(root, 8) - 500) < 0.5, "the row's bottom on the view's", 1000)


def test_steps_faster_than_the_slide_keep_the_row_in_view(load):
    root, _ = load(LIST)
    worst = 0
    for i in range(1, 13):
        root.setProperty("index", i)
        pump(90)
        worst = max(worst, shown(root, i) - 500)
    assert worst < 100, f"the row ran {worst:.0f} px past the edge: the view fell behind the cursor"


def test_the_last_row_far_off_screen_is_reached(load):
    root, _ = load(LIST)
    root.setProperty("index", 59)
    until(lambda: abs(shown(root, 59) - 500) < 0.5, "a row with no delegate yet is found and shown", 1000)


def test_a_cursor_moved_with_the_model_lands_at_once(load):
    root, _ = load(LIST)
    root.setProperty("rows", 80)
    root.setProperty("index", 50)
    pump(20)
    assert 0 <= shown(root, 50) <= 500, "placed in the same frame, no slide across fifty rows"


def test_a_held_d_pad_scrubs_further_the_longer_it_is_held(load):
    root, _ = load(SCRUB)
    scrub = root.findChild(QObject, "scrub")
    invoke(scrub, "stepBy", 1, False)
    assert root.property("pos") == 10000
    pump(500)
    invoke(scrub, "stepBy", 1, True)
    assert root.property("pos") - 10000 > 13000, "half a second in, the step has grown by about 2^0.5"
    invoke(scrub, "stepBy", -1, False)
    assert root.property("pos") - 10000 > 13000 - 10000 - 1, "a fresh press starts again at one step"


def test_the_stick_held_at_full_tilt_scrubs_faster_and_faster(load):
    root, api = load(SCRUB)
    api.pad.set_right_x(1.0)
    pump(300)
    first = root.property("pos")
    pump(300)
    second = root.property("pos") - first
    api.pad.set_right_x(0.0)
    assert second > first * 1.1, f"{first:.0f} ms in the first 300 ms, {second:.0f} in the next: no faster"
