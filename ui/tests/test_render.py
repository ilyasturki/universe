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
    assert lit_fraction(image, api.theme.ground) > 0.01
    assert image.pixelColor(4, 4).name() == api.theme.ground
    root = window.property("contentItem").childItems()[0].property("item")
    top = root.property("topPage")
    assert root.property("depth") == 1 and top is not None and top.property("sectionId") == "themes", "a switch lands on the new look's Themes page"
    assert api.theme.landing == "", "taken once"
    api.theme.set("reprise")
    settle(window)
    assert lit_fraction(window.grabWindow(), api.theme.ground) > 0.01
    root = window.property("contentItem").childItems()[0].property("item")
    page = root.property("activePage")
    assert root.property("tabIndex") == 4 and page is not None and page.property("section") == page.property("themesSection")
    window.close()
    pump(50)


def test_the_media_tab_and_the_screenshots_page(api, fake):
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(3)
    settle(window)
    page = root.property("activePage")
    def rows():
        value = page.property("rows")
        return value.toVariant() if hasattr(value, "toVariant") else value

    def current():
        value = page.property("current")
        return value.toVariant() if hasattr(value, "toVariant") else value

    assert root.property("tabIndex") == 3 and page is not None and rows() and current()["kind"] in ("shot", "recording", "journal")
    before = lit_fraction(window.grabWindow(), api.theme.ground)
    assert before > 0.05
    page.setProperty("kindIndex", 1)
    pump(100)
    assert all(r["kind"] == "shot" for r in rows()) and rows()
    page.setProperty("gameFilter", "the-technomancer")
    pump(100)
    assert all(r["gameId"] == "the-technomancer" for r in rows()) and rows()
    page.open()
    pump(100)
    assert page.property("lightbox") is True and page.property("modal") is True
    page.setProperty("lightbox", False)
    game = page.property("currentGame")
    assert game is not None and game.property("id") == "the-technomancer"
    page.screenshotsRequested.emit(game, current()["name"])
    settle(window)
    assert root.property("subOpen") is True and root.property("subSource") == "pages/ScreenshotsPage.qml"
    shots = api.screens.shots
    assert shots.gameId == "the-technomancer" and shots.count > 0, "the sub-page loaded the game's shots"
    assert lit_fraction(window.grabWindow(), api.theme.ground) > 0.05
    root.closeSub()
    settle(window)
    assert root.property("subOpen") is False
    window.close()
    pump(50)


def test_the_switch2_album_holds_the_shots_too(api):
    from PySide6.QtCore import Q_ARG, QMetaObject

    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/AlbumPage.qml"), Q_ARG("QVariant", {}))
    settle(window)
    top = root.property("topPage")
    assert top is not None
    shown = top.property("shown")
    shown = shown.toVariant() if hasattr(shown, "toVariant") else shown
    kinds = {r["kind"] for r in shown}
    assert kinds == {"shot", "recording"} and [r["when"] for r in shown] == sorted((r["when"] for r in shown), reverse=True)
    top.setProperty("kindFilter", "shot")
    pump(100)
    shown = top.property("shown")
    shown = shown.toVariant() if hasattr(shown, "toVariant") else shown
    assert shown and all(r["kind"] == "shot" for r in shown)
    top.play()
    pump(100)
    assert top.property("viewing") is True
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


def test_the_cursor_follows_the_game_through_its_session(api, fake, monkeypatch):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = root.property("activePage")
    overlay = window.findChild(QObject, "launchOverlay")
    assert home.property("currentGame").property("id") == "the-technomancer"
    for ident in ("dead-cells", "mirrors-edge"):  # a row on the rail, then a first play that has none yet
        while overlay.property("running"):
            pump(20)
        before = home.property("currentGame").property("id")
        QMetaObject.invokeMethod(root, "launchGame", Q_ARG("QVariant", api.allGames.byId(ident)))
        wait_for(fake.sessionStarted, 3000)
        pump(50)
        assert home.property("currentGame").property("id") == before
        fake.stop("")
        wait_for(fake.sessionEnded, 3000)
        pump(50)
        assert root.property("playingId") == ""
        assert home.property("currentGame").property("id") == ident
    window.close()
    pump(50)


def test_the_switch2_home_row_follows_the_game_too(api, fake, monkeypatch):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = root.findChild(QObject, "homePage")
    assert home.property("currentGame").property("id") == "the-technomancer"
    QMetaObject.invokeMethod(root, "launch", Q_ARG("QVariant", api.allGames.byId("dead-cells")))
    wait_for(fake.sessionStarted, 3000)
    pump(50)
    assert home.property("currentGame").property("id") == "the-technomancer" and home.property("index") == 1
    fake.stop("")
    wait_for(fake.sessionEnded, 3000)
    pump(50)
    assert home.property("currentGame").property("id") == "dead-cells" and home.property("index") == 0
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
