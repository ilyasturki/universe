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
        return page.property("rows").toVariant()

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


def test_the_screenshots_page_puts_the_running_sessions_shots_first(api, fake, monkeypatch):
    from PySide6.QtCore import QObject
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/ScreenshotsPage.qml", {"game": game})
    settle(window)
    page = window.findChild(QObject, "screenshotsPage")
    assert page is not None, "the screenshots page is up"
    earlier = page.property("rows").toVariant()
    assert earlier and page.property("since") == "" and page.property("mine") == 0, "no session: one run, no headings"
    fake.launch("the-technomancer", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    api.home.screenshot()
    wait_for(api.home.screenshotTaken, 3000)
    wait_for(api.screens.shots.rowsChanged, 3000)
    pump(100)
    assert page.property("since") != "" and page.property("mine") == 1, "the playing game's page splits at the session's start"
    rows = page.property("rows").toVariant()
    assert len(rows) == len(earlier) + 1 and rows[0]["name"] not in {r["name"] for r in earlier}, "the new shot leads"
    api.universe.stop(api.universe.currentSession["session_id"])
    wait_for(api.universe.sessionEnded, 5000)
    pump(100)
    assert page.property("since") == "", "the session over, one run again"
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
    shown = top.property("shown").toVariant()
    kinds = {r["kind"] for r in shown}
    assert kinds == {"shot", "recording"} and [r["when"] for r in shown] == sorted((r["when"] for r in shown), reverse=True)
    top.setProperty("kindFilter", "shot")
    pump(100)
    shown = top.property("shown").toVariant()
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


def test_the_install_pages_render_a_running_install_in_both_looks(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    sources = api.screens.sources
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")

    def js(obj, name):
        value = obj.property(name)
        return value.toVariant() if hasattr(value, "toVariant") else value

    root.setProperty("tabIndex", 3)
    pump(100)
    page = root.property("activePage")
    page.setProperty("section", page.property("installSection"))
    pump(400)
    content = js(page, "content")
    groups = content["groups"]
    assert [content["rows"][i]["label"] for i in groups[0]["rows"]] == ["Disco Elysium"], "the paused download sits in the Installing card"
    assert [g["title"] for g in groups] == ["Installing", "Installed", "Owned, not installed"]
    assert groups[0]["meta"] == "1 paused" and " GB · /mnt/games/PC" in groups[1]["meta"]
    row = next(i for i, r in enumerate(sources.rows) if r["title"] == "Stardew Valley")
    sources.install(row)
    pump(300)
    assert js(page, "content")["groups"][0]["meta"] == "1 running · 1 paused"
    assert sources.cancel()
    wait_for(fake.jobFinished, 5000)
    pump(300)
    assert js(page, "content")["groups"][0]["meta"] == "2 paused"

    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/InstallPage.qml"), Q_ARG("QVariant", {}))
    pump(500)
    install = root.property("topPage")
    assert [c["label"] for c in js(install, "cells") if c.get("heading")] == ["Owned, not installed"]
    assert [l["label"] for l in js(install, "lines") if l.get("heading")] == ["Installing", "Installed"]
    assert [h["glyph"] for h in js(install, "hints")] == ["Y", "B", "A"]
    QTest.keyClick(window, Qt.Key.Key_E)  # RB: Manage
    pump(100)
    assert install.property("tab") == 1
    sources.install(row)
    pump(300)
    assert "X" in [h["glyph"] for h in js(install, "hints")], "X cancels while a job runs"
    other = next(i for i, r in enumerate(sources.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    said = []
    sources.message.connect(said.append)
    assert sources.install(other) == "" and said == ["Installing Stardew Valley first — cancel it or wait"]
    assert sources.job["game"] == sources.rows[row]["id"], "one job at a time"
    QTest.keyClick(window, Qt.Key.Key_I)
    wait_for(fake.jobFinished, 5000)
    pump(300)
    assert sources.job["cancelled"] and next(r for r in sources.rows if r["title"] == "Stardew Valley")["partial"]
    window.close()
    pump(50)


def test_the_right_stick_pages_the_grids_in_both_looks(api):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    def title(page):
        game = page.property("currentGame")
        return game.property("title") if game is not None else None

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(50)

    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(1)
    settle(window)
    page = root.property("activePage")
    # Eight games on eight columns: the add tile alone on the second row.
    click(Qt.Key.Key_BracketLeft)
    first = title(page)
    assert first and page.property("onAddTile") is False, "a screenful up lands on the first row"
    click(Qt.Key.Key_BracketLeft)
    assert title(page) == first, "the first row is a clamp"
    click(Qt.Key.Key_Right, 3)
    click(Qt.Key.Key_BracketRight)
    assert page.property("onAddTile") is True, "down past the last game's row lands on the last cell"
    click(Qt.Key.Key_BracketRight, 3)
    assert page.property("onAddTile") is True
    click(Qt.Key.Key_BracketLeft)
    assert title(page) == first, "back up from the last cell: its own column, the first"
    window.close()
    pump(50)

    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/AlbumPage.qml"), Q_ARG("QVariant", {}))
    settle(window)
    top = root.property("topPage")
    first = top.property("current").toVariant()
    click(Qt.Key.Key_BracketRight)
    assert top.property("current").toVariant() != first
    click(Qt.Key.Key_BracketLeft)
    assert top.property("current").toVariant() == first
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    for source, args, keys in (("pages/SettingsPage.qml", {"section": "launch"}, (Qt.Key.Key_Right,)),
                               ("pages/AllSoftwarePage.qml", {}, ())):
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
        settle(window)
        for key in keys + (Qt.Key.Key_BracketRight, Qt.Key.Key_BracketRight, Qt.Key.Key_BracketLeft, Qt.Key.Key_Left, Qt.Key.Key_BracketRight):
            click(key)
        click(Qt.Key.Key_Escape, 2)
    assert warnings == []
    window.close()
    pump(50)

def test_the_artwork_page_opens_on_a_slot_and_lists_its_candidates(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    QMetaObject.invokeMethod(root, "openSub", Q_ARG("QVariant", "pages/ArtworkPage.qml"), Q_ARG("QVariant", {"game": game, "slot": "logo"}))
    settle(window)
    form = api.screens.artwork
    assert root.property("subOpen") is True and form.gameId == "the-technomancer"
    page = root.findChild(QObject, "artworkPage")
    assert page is not None and page.property("level") == "browser", "opened on a slot, the page went straight to its candidates"
    if not form.candidates:
        wait_for(form.candidatesChanged, 3000)
        pump(100)
    assert form.candidatesSlot == "logo" and form.candidates
    assert lit_fraction(window.grabWindow(), api.theme.ground) > 0.05
    page.closeBrowser()
    pump(100)
    assert page.property("level") == "slots" and page.property("index") == 4
    page.openMenu()
    pump(100)
    assert page.property("modal") is True
    window.close()
    pump(50)


def test_the_switch2_artwork_page_opens_a_slot_with_what_shows_first(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject

    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/ArtworkPage.qml"), Q_ARG("QVariant", {"gameId": "dead-cells"}))
    settle(window)
    top = root.property("topPage")
    form = api.screens.artwork
    depth = root.property("depth")
    assert form.gameId == "dead-cells" and [s["slot"] for s in top.property("slots")][:2] == ["box_front", "square"]
    top.setProperty("index", 1)
    top.open()
    settle(window)
    if not form.candidates:
        wait_for(form.candidatesChanged, 3000)
        pump(100)
    top = root.property("topPage")
    assert root.property("depth") == depth + 1 and top.property("slot") == "square" and form.candidatesSlot == "square"
    cells = top.property("cells").toVariant()
    assert cells[0]["kind"] == "now" and cells[1]["kind"] == "candidate" and len(cells) == 1 + len(form.candidates)
    top.setProperty("cellIndex", 2)
    top.activate()
    wait_for(fake.mediaChanged, 3000)
    pump(200)
    cells = top.property("cells").toVariant()
    assert cells[1]["kind"] == "under" and form.slot("square")["kind"] == "picked", "a pick puts the default under it"
    assert top.property("cellIndex") == 3, "the ring stays on the candidate that was picked"
    assert lit_fraction(window.grabWindow(), api.theme.ground) > 0.02
    window.close()
    pump(50)


def test_b_held_asks_to_quit_in_both_looks(api):
    from PySide6.QtCore import QObject, Qt
    from PySide6.QtTest import QTest

    def hold(ms):
        QTest.keyPress(window, Qt.Key.Key_Escape)
        pump(ms)
        QTest.keyRelease(window, Qt.Key.Key_Escape)
        pump(50)

    engine, window = render(api, activate=True)
    confirm = window.findChild(QObject, "confirm")
    hold(150)
    assert confirm.property("open") is False, "a tap is a tap"
    hold(600)
    assert confirm.property("open") is True and confirm.property("message") == "Quit Universe?"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(100)
    assert confirm.property("open") is False, "B on the question stays"
    api.theme.set("switch2")
    settle(window)
    dialog = window.findChild(QObject, "dialog")
    hold(600)
    assert dialog.property("open") is True and dialog.property("message") == "Quit Universe?"
    hold(600)
    assert dialog.property("open") is False, "held on the question: B closes it and the hold asks nothing more"
    window.close()
    pump(50)


def test_reprise_about_shows_the_build(api, fake):
    engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(4)
    settle(window)
    page = root.property("activePage")
    page.setProperty("section", page.property("aboutSection"))
    pump(100)
    content = page.property("content").toVariant()
    rows = {r["label"]: r["display"] for r in content["rows"]}
    assert rows["Universe"] == fake.version() and rows["Look"] == "Reprise" and rows["Library"].endswith(" games")
    assert page.property("acceptLabel") == "", "nothing to select"
    window.close()
    pump(50)
