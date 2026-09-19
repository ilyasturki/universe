from pathlib import Path

import pytest

FORMAT_JS = Path(__file__).resolve().parents[1] / "universe_ui" / "qml" / "core" / "Format.js"


def js_bytes(app, values):
    from PySide6.QtCore import QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine

    engine = QQmlEngine()
    component = QQmlComponent(engine)
    calls = ", ".join(f"Format.bytes({v})" for v in values)
    component.setData(f'import QtQuick\nimport "{FORMAT_JS.as_uri()}" as Format\nQtObject {{ property var out: [{calls}] }}\n'.encode(), QUrl("file:///format.qml"))
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    return obj.property("out").toVariant()


def test_the_qml_size_formatter_is_the_hosts(app):
    # The Install page shows the host's sizeText next to the confirm dialog's JS sizes: one spelling
    from universe_ui.screens.media import _size

    values = [0, 512, 1023, 1024, 1500, 1536, 10 * 1024**2, 3.4 * 1024**3, 2 * 1024**4]
    assert js_bytes(app, values) == [_size(v) for v in values]
    assert js_bytes(app, ["-5", "undefined", "null"]) == [_size(0)] * 3


@pytest.mark.parametrize("fmt", ["svg", "png", "jpeg", "webp"])
def test_image_formats_the_themes_need_are_loadable(app, fmt):
    # The platform icons are SVGs: the plugin comes with QT_PLUGIN_PATH, which only wrapQtAppsHook sets on its own
    from PySide6.QtGui import QImageReader

    assert fmt.encode() in QImageReader.supportedImageFormats()
