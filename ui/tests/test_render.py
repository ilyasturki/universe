from PySide6.QtCore import QUrl
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuick import QQuickWindow  # noqa: F401  (rootObjects() down-cast, for grabWindow)

from conftest import pump
from universe_ui import host

GROUND = (0x0E, 0x0F, 0x13)
WHITE_GROUND = (0xEB, 0xEB, 0xEB)


def lit_fraction(image, ground=GROUND):
    small = image.scaled(96, 54)
    lit = 0
    for y in range(small.height()):
        for x in range(small.width()):
            c = small.pixelColor(x, y)
            if abs(c.red() - ground[0]) + abs(c.green() - ground[1]) + abs(c.blue() - ground[2]) > 60:
                lit += 1
    return lit / (small.width() * small.height())


def render(api, width=1280, height=720, settle=2500):
    engine = QQmlApplicationEngine()
    for p in host.qml_import_paths() if host.qt_paths_unset() else []:
        engine.addImportPath(p)
    engine.rootContext().setContextProperty("api", api)
    engine.load(QUrl.fromLocalFile(str(host.QML_DIR / "main.qml")))
    assert engine.rootObjects(), "main.qml failed to load"
    window = engine.rootObjects()[0]
    api.attachWindow(window)
    window.setWidth(width)
    window.setHeight(height)
    pump(settle)
    image = window.grabWindow()
    return engine, window, image


def test_themes_render_and_switch_live(api):
    engine, window, image = render(api)
    assert image.width() == 1280 and image.height() == 720
    assert lit_fraction(image) > 0.05
    api.theme.set("switch2-white")
    pump(2500)
    image = window.grabWindow()
    assert lit_fraction(image, WHITE_GROUND) > 0.05
    assert image.pixelColor(4, 4).getRgb()[:3] == WHITE_GROUND
    api.theme.set("switch2-black")
    pump(400)
    assert window.grabWindow().pixelColor(4, 4).red() < 0x40, "black ground after the palette switch"
    api.theme.set("reprise")
    pump(1500)
    assert lit_fraction(window.grabWindow()) > 0.05
    window.close()
    pump(50)


def test_a_session_running_at_startup_is_the_running_view(api, fake):
    from PySide6.QtCore import QObject

    fake.launch("the-technomancer", "")
    assert fake.currentSession
    engine = QQmlApplicationEngine()
    for p in host.qml_import_paths() if host.qt_paths_unset() else []:
        engine.addImportPath(p)
    engine.rootContext().setContextProperty("api", api)
    engine.load(QUrl.fromLocalFile(str(host.QML_DIR / "main.qml")))
    assert engine.rootObjects(), "main.qml failed to load"
    window = engine.rootObjects()[0]
    api.attachWindow(window)
    window.requestActivate()
    pump(800)
    overlay = window.findChild(QObject, "launchOverlay")
    assert overlay is not None
    assert overlay.property("playing") is True and overlay.property("running") is True
    assert overlay.property("activeFocus") is True
    pump(2500)
    assert fake.currentSession is None
    assert overlay.property("running") is False and overlay.property("activeFocus") is False
    window.close()
    pump(50)


def test_signals_end_the_loop_while_it_idles(app):
    import os
    import signal
    import threading

    from PySide6.QtCore import QEventLoop, QTimer

    loop = QEventLoop()
    got = []
    notifier = host.quit_on_signals(app, on_signal=lambda: (got.append(True), loop.quit()))
    try:
        # From another thread, so the main thread is asleep in Qt when the signal lands.
        threading.Timer(0.3, os.kill, (os.getpid(), signal.SIGTERM)).start()
        QTimer.singleShot(4000, loop.quit)
        loop.exec()
        assert got
    finally:
        signal.set_wakeup_fd(-1)
        signal.signal(signal.SIGINT, signal.default_int_handler)
        signal.signal(signal.SIGTERM, signal.SIG_DFL)
        notifier.setEnabled(False)
        notifier.deleteLater()
