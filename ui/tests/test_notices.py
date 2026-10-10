from pathlib import Path

import pytest
from uitest import until

CORE = Path(__file__).resolve().parents[1] / "universe_ui" / "qml" / "core"
INFO_MS = 50
ERROR_MS = 4 * INFO_MS


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
    Component.onCompleted: {{ Notices.infoMs = {INFO_MS}; Notices.errorMs = {ERROR_MS}; Notices.gap.interval = 10; }}
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


# Each step is (call, text, key); a dict after a step is the queue it must leave; an int, the least ms before it empties.
QUEUE = {
    "waits its turn": [
        ("show", "Removed Control", ""),
        ("show", "Journal: writing Control…", ""),
        {"current": "Removed Control", "waiting": ["Journal: writing Control…"]},
    ],
    "a follow-up takes its own place": [
        ("show", "Journal: writing Control…", "journal:1"),
        ("show", "Screenshot saved", ""),
        ("show", "Journal: writing Hades…", "journal:2"),
        ("show", "Journal: Control, 2 h", "journal:1"),
        ("fail", "Journal failed: quota", "journal:2"),
        {"current": "Journal: Control, 2 h", "waiting": ["!Journal failed: quota"]},
    ],
    "a burst keeps only the newest waiting": [
        ("show", "A is B now", ""),
        ("show", "X is Y now", ""),
        ("show", "L1 is R1 now", ""),
        ("show", "Pad set up", ""),
        {"current": "A is B now", "waiting": ["Pad set up"]},
    ],
    "an info does not bury a waiting error": [
        ("show", "Screenshot saved", ""),
        ("fail", "Could not launch Control", ""),
        ("show", "MangoHud hidden", ""),
        {"current": "Screenshot saved", "waiting": ["!Could not launch Control"]},
        ("fail", "Could not launch Hades", ""),
        {"current": "Screenshot saved", "waiting": ["!Could not launch Hades"]},
    ],
    "a repeat is said once": [
        ("show", "MangoHud shown", ""),
        ("show", "MangoHud shown", ""),
        {"current": "MangoHud shown", "waiting": []},
        ("show", "Nothing new", ""),
        ("show", "Nothing new", ""),
        {"current": "MangoHud shown", "waiting": ["Nothing new"]},
    ],
    "an error stays longer": [("fail", "Could not launch Control", ""), 2 * INFO_MS],
}


@pytest.mark.parametrize("steps", QUEUE.values(), ids=QUEUE.keys())
def test_the_queue_shows_each_notice_in_turn_then_empties(notices, steps):
    from PySide6.QtCore import QElapsedTimer

    obj, _ = notices
    shown = QElapsedTimer()
    shown.start()
    at_least = 0
    for step in steps:
        if isinstance(step, dict):
            assert state(obj) == step
        elif isinstance(step, int):
            at_least = step
        else:
            call(obj, *step)
    until(lambda: state(obj) == {"current": None, "waiting": []})
    assert shown.elapsed() > at_least
