from PySide6.QtCore import QUrl
from PySide6.QtGui import QColor
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuick import QQuickWindow  # noqa: F401  (rootObjects() down-cast, for grabWindow)

from conftest import pump, wait_for
from universe_ui import host


def lit_fraction(image, ground):
    small = image.scaled(96, 54)
    r, g, b = QColor(ground).getRgb()[:3]
    lit = 0
    for y in range(small.height()):
        for x in range(small.width()):
            c = small.pixelColor(x, y)
            if abs(c.red() - r) + abs(c.green() - g) + abs(c.blue() - b) > 60:
                lit += 1
    return lit / (small.width() * small.height())


def settle(window):
    wait_for(window.frameSwapped, 3000)
    pump(200)


def render(api, width=1280, height=720, activate=False):
    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("api", api)
    engine.load(QUrl.fromLocalFile(str(host.QML_DIR / "main.qml")))
    assert engine.rootObjects(), "main.qml failed to load"
    window = engine.rootObjects()[0]
    api.attachWindow(window)
    window.setWidth(width)
    window.setHeight(height)
    if activate:
        window.requestActivate()
    settle(window)
    return engine, window


def test_themes_render_and_switch_live(api):
    engine, window = render(api)
    image = window.grabWindow()
    assert image.width() == 1280 and image.height() == 720
    assert lit_fraction(image, api.theme.ground) > 0.05
    api.theme.set("switch2")
    settle(window)
    image = window.grabWindow()
    assert lit_fraction(image, api.theme.ground) > 0.05
    assert image.pixelColor(4, 4).name() == api.theme.ground
    api.theme.set("reprise")
    settle(window)
    assert lit_fraction(window.grabWindow(), api.theme.ground) > 0.05
    window.close()
    pump(50)


def test_a_session_running_at_startup_is_home_with_the_game_pinned(api, fake):
    from PySide6.QtCore import QObject

    fake.launch("mirrors-edge", "")
    wait_for(fake.launched, 3000)
    assert fake.currentSession
    engine, window = render(api, activate=True)
    overlay = window.findChild(QObject, "launchOverlay")
    assert overlay is not None
    assert overlay.property("running") is False
    root = window.property("contentItem").childItems()[0].property("item")
    assert root.property("playingId") == "mirrors-edge"
    home = root.property("activePage")
    assert home is not None and home.property("currentGame").property("id") == "mirrors-edge"
    assert home.property("playLabel") == "Resume"
    wait_for(fake.sessionEnded, 5000)
    pump(50)
    assert fake.currentSession is None
    assert root.property("playingId") == ""
    window.close()
    pump(50)


def test_a_launch_holds_the_poster_until_the_window_is_shown(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    overlay = window.findChild(QObject, "launchOverlay")
    QMetaObject.invokeMethod(root, "launchGame", Q_ARG("QVariant", api.allGames.byId("control")))
    assert overlay.property("running") is True and root.property("launching") is True
    wait_for(fake.sessionStarted, 3000)
    assert overlay.property("waiting") is True and fake.currentSession["id"] == "control"
    assert fake.core.last_splash.endswith("splash-control.bgrx")
    with open(fake.core.last_splash, "rb") as f:
        header = f.readline().decode().split()
        assert [int(v) for v in header] == [round(window.width() * window.devicePixelRatio()), round(window.height() * window.devicePixelRatio())]
        assert len(f.read()) == int(header[0]) * int(header[1]) * 4
    assert wait_for(fake.sessionShown, 3000)[1] is True
    wait_for(overlay.runningChanged, 3000)
    assert overlay.property("running") is False and root.property("launching") is False
    assert root.property("playingId") == "control"
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
