from pathlib import Path

import pytest

from conftest import pump

CORE = Path(__file__).resolve().parents[1] / "universe_ui" / "qml" / "core"


@pytest.fixture
def notices(app):
    from PySide6.QtCore import QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine

    engine = QQmlEngine()
    component = QQmlComponent(engine)
    component.setData(
        f"""import QtQuick
import "{CORE.as_uri()}"
QtObject {{
    Component.onCompleted: {{ Notices.infoMs = 150; Notices.errorMs = 400; }}
    function show(text, key) {{ Notices.show(text, key); }}
    function fail(text, key) {{ Notices.fail(text, key); }}
    function state() {{
        var line = function (m) {{ return m ? (m.error ? "!" : "") + m.text : null; }};
        return {{ current: line(Notices.current), waiting: Notices.waiting.map(line) }};
    }}
}}
""".encode(),
        QUrl("file:///notices.qml"),
    )
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    yield obj, engine


def call(obj, name, *args):
    from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, Qt

    ret = Q_RETURN_ARG("QVariant")
    out = QMetaObject.invokeMethod(obj, name, Qt.DirectConnection, ret, *(Q_ARG("QVariant", a) for a in args))
    return out.toVariant() if hasattr(out, "toVariant") else out


def state(obj):
    return call(obj, "state")


def test_messages_wait_their_turn(notices):
    obj, _ = notices
    call(obj, "show", "Removed Control", "")
    call(obj, "show", "Journal: writing Control…", "")
    assert state(obj) == {"current": "Removed Control", "waiting": ["Journal: writing Control…"]}
    pump(150 + 320 + 60)
    assert state(obj) == {"current": "Journal: writing Control…", "waiting": []}
    pump(150 + 60)
    assert state(obj)["current"] is None


def test_a_follow_up_takes_its_own_place(notices):
    obj, _ = notices
    call(obj, "show", "Journal: writing Control…", "journal:1")
    call(obj, "show", "Screenshot saved", "")
    call(obj, "show", "Journal: writing Hades…", "journal:2")
    call(obj, "show", "Journal: Control, 2 h", "journal:1")
    call(obj, "fail", "Journal failed: quota", "journal:2")
    assert state(obj) == {"current": "Journal: Control, 2 h", "waiting": ["Screenshot saved", "!Journal failed: quota"]}


def test_a_repeat_is_said_once(notices):
    obj, _ = notices
    call(obj, "show", "MangoHud shown · Control", "")
    call(obj, "show", "MangoHud shown · Control", "")
    call(obj, "show", "Nothing new in Lutris", "")
    call(obj, "show", "Nothing new in Lutris", "")
    assert state(obj) == {"current": "MangoHud shown · Control", "waiting": ["Nothing new in Lutris"]}


def test_an_error_stays_longer(notices):
    obj, _ = notices
    call(obj, "fail", "Could not launch Control", "")
    pump(150 + 60)
    assert state(obj)["current"] == "!Could not launch Control"
    pump(400)
    assert state(obj)["current"] is None
