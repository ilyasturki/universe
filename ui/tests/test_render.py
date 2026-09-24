import os

from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QObject, Qt, QUrl
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


def count_frames(window, ms):
    frames = []
    window.frameSwapped.connect(lambda: frames.append(1))
    pump(ms)
    window.frameSwapped.disconnect()
    return len(frames)


def test_the_scene_holds_still_behind_the_game(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    _engine, window = render(api)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(1500)
    assert api.home.shown == "game"
    covered = count_frames(window, 1500)
    api.home.toLauncher()
    pump(1200)
    assert api.home.shown == "launcher"
    shown = count_frames(window, 1500)
    assert covered <= 3 < shown, f"{covered} frames under the game, {shown} with the launcher up: the badge pulses and the hero drifts only when seen"


def test_themes_render_and_switch_live(api):
    _engine, window = render(api)
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
    assert root.property("tabIndex") == root.property("settingsTab") and page is not None and page.property("sectionId") == "themes"
    window.close()
    pump(50)


def test_the_media_tab_and_the_screenshots_page(api, fake):
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(1)
    settle(window)
    if api.screens.media.loading:
        assert wait_for(api.screens.media.rowsChanged, 5000) is not None
        pump(50)
    page = root.property("activePage")

    def read(name):
        value = page.property(name)
        return value.toVariant() if hasattr(value, "toVariant") else value

    def rows():
        return read("rows")

    def current():
        return read("current")

    assert root.property("tabIndex") == 1 and page is not None and rows() and current()["kind"] in ("shot", "recording", "journal")
    assert {r["kind"] for r in rows()} == {"shot", "recording", "journal"}, "one grid, every kind, no filter"
    assert [h["glyph"] for h in read("hints")] == ["A", "Start", "B"]
    before = lit_fraction(window.grabWindow(), api.theme.ground)
    assert before > 0.05
    page.setProperty("index", next(i for i, r in enumerate(rows()) if r["kind"] == "shot" and r["gameId"] == "the-technomancer"))
    page.open()
    pump(100)
    assert page.property("lightbox") is True and page.property("modal") is True
    page.setProperty("lightbox", False)
    game = page.property("currentGame")
    assert game is not None and game.property("id") == "the-technomancer"
    root.openSub("pages/ScreenshotsPage.qml", {"game": game, "name": current()["name"]})
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


def test_the_tab_bar_search_finds_the_settings_under_the_games(api, fake):
    from PySide6.QtTest import QTest

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.openSearch()
    settle(window)
    overlay = root.property("focusTarget")
    search = api.screens.search
    while not search.ready:
        assert wait_for(search.readyChanged, 5000) is not None
    assert [s["id"] for s in search.sections][:3] == ["launch", "runners", "controller"], "indexed with Reprise's sections before Settings ever opened"
    overlay.setProperty("query", "techno")
    pump(100)
    assert overlay.property("hasGames") is True and overlay.property("hasSettings") is False, "a title alone: the cover, not the game's every setting"
    overlay.setProperty("query", "quit")
    pump(100)
    assert {"page": "section", "id": "about", "key": "", "module": ""} in [r["target"] for r in search.results], "Quit lives in About: its word finds About"
    overlay.setProperty("query", "vrr")
    pump(100)
    assert overlay.property("hasSettings") is True and overlay.property("hasGames") is False
    overlay.toResults()
    pump(100)
    assert overlay.property("zone") == "settings" and [h["label"] for h in overlay.property("hints").toVariant()] == ["Open", "Close"]
    QTest.keyClick(window, Qt.Key.Key_Return)
    settle(window)
    page = root.property("activePage")
    assert root.property("searchOpen") is False and root.property("tabIndex") == root.property("settingsTab")
    cards = next(c for c in page.findChildren(QObject) if c.property("focusRect") is not None)
    assert page.property("sectionId") == "launch" and cards.property("currentRow")["label"] == "Adaptive sync", "the hit's row, on its section"
    window.close()
    pump(50)


def test_the_screenshots_page_puts_the_running_sessions_shots_first(api, fake, monkeypatch):
    from PySide6.QtCore import QObject

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/ScreenshotsPage.qml", {"game": game})
    settle(window)
    page = window.findChild(QObject, "screenshotsPage")
    assert page is not None, "the screenshots page is up"
    earlier = page.property("rows").toVariant()
    assert earlier and page.property("since") == "" and page.property("mine") == 0, "no session: one run, no headings"
    grid = next(c for c in page.findChildren(QObject) if c.property("cellHeight") is not None)
    card = QMetaObject.invokeMethod(grid, "currentCard", Qt.DirectConnection, Q_RETURN_ARG("QVariant"))
    picture = card.property("height") - card.property("captionHeight")
    assert abs(picture - card.property("width") * 9 / 16) < 1, "the cell makes room for the date line: the picture stays 16:9, whole"
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
    _engine, window = render(api, activate=True)
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
    _engine, window = render(api, activate=True)
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
    _engine, window = render(api, activate=True)
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
    _engine, window = render(api, activate=True)
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

    _engine, window = render(api, activate=True)
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
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")

    def js(obj, name):
        value = obj.property(name)
        return value.toVariant() if hasattr(value, "toVariant") else value

    root.setProperty("tabIndex", root.property("settingsTab"))
    pump(100)
    page = root.property("activePage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "install"))
    pump(400)
    content = js(page, "content")
    groups = content["groups"]
    assert [content["rows"][i]["label"] for i in groups[0]["rows"]] == ["Disco Elysium"], "the paused download sits in the Installing card"
    assert [g["title"] for g in groups] == ["Installing", "Updates", "Installed", "Owned, not installed"], "the pending updates fold into Install"
    assert [content["rows"][i]["label"] for i in groups[1]["rows"]] == ["Update everything"]
    assert groups[0]["meta"] == "1 paused" and " GB · /mnt/games/PC" in groups[2]["meta"]
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
    assert [line["label"] for line in js(install, "lines") if line.get("heading")] == ["Installing", "Installed"]
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


def test_the_reprise_library_leads_with_hearts_and_y_hearts_the_game_under_the_cursor(api):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def title():
        game = page.property("currentGame")
        return game.property("title") if game is not None else None

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(80)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("libraryTab"))
    settle(window)
    page = root.property("activePage")
    assert title() == "Dead Cells", "the hearted games first, by title"
    click(Qt.Key.Key_Right, 2)
    assert title() == "Batman: Arkham Origins", "then the rest, by title"
    assert [h["glyph"] for h in page.property("hints").toVariant()] == ["A", "Start", "B"]
    click(Qt.Key.Key_F)
    assert api.allGames.byId("batman-arkham-origins").favorite is True
    assert title() == "Batman: Arkham Origins", "the cursor follows the game to its place among the hearts"
    click(Qt.Key.Key_Right)
    assert title() == "Dead Cells"
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
    root.goToTab(root.property("libraryTab"))
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
    for source, args, keys in (("pages/SettingsPage.qml", {"section": "launch"}, (Qt.Key.Key_Right,)), ("pages/AllSoftwarePage.qml", {}, ())):
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
        settle(window)
        for key in (*keys, Qt.Key.Key_BracketRight, Qt.Key.Key_BracketRight, Qt.Key.Key_BracketLeft, Qt.Key.Key_Left, Qt.Key.Key_BracketRight):
            click(key)
        click(Qt.Key.Key_Escape, 2)
    assert warnings == []
    window.close()
    pump(50)


def test_the_artwork_page_opens_on_a_slot_and_lists_its_candidates(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    _engine, window = render(api, activate=True)
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
    assert lit_fraction(window.grabWindow(), api.theme.ground) > 0.03
    page.back()
    settle(window)
    assert root.property("subOpen") is False, "opened on a slot, B leaves the page rather than showing the cards"
    QMetaObject.invokeMethod(root, "openSub", Q_ARG("QVariant", "pages/ArtworkPage.qml"), Q_ARG("QVariant", {"game": game}))
    settle(window)
    page = root.findChild(QObject, "artworkPage")
    assert page.property("level") == "slots"
    page.setProperty("index", 0)
    page.moveAcross(1)
    page.moveDown()
    page.moveAcross(-1)
    page.moveAcross(1)
    assert page.property("index") == 3, "the box front remembers the row it was left from"
    page.openMenu()
    pump(100)
    assert page.property("modal") is True
    window.close()
    pump(50)


def test_the_settings_artwork_button_asks_then_fetches_and_stops(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    fake.core._game("control")["media"].pop("logo")
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.setProperty("tabIndex", root.property("settingsTab"))
    pump(100)
    page = root.property("activePage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "artwork"))
    settle(window)
    store = api.screens.artworkOverview
    assert store.missingGames == 1
    QTest.keyClick(window, Qt.Key.Key_Right)
    QTest.keyClick(window, Qt.Key.Key_Up)
    pump(50)
    overview = page.findChild(QObject, "artworkOverview")
    assert overview is not None and overview.property("onButton") is True
    assert overview.property("buttonLabel") == "Fetch missing art"
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(100)
    assert next(h["label"] for h in page.property("hints").toVariant()) == "Select", "the confirm has the focus"
    assert store.job is None, "nothing runs before the confirm"
    QTest.keyClick(window, Qt.Key.Key_Return)
    while store.job is None or store.job["total"] == 0:
        wait_for(store.jobChanged)
    assert overview.property("buttonLabel").startswith("Stop · ")
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(20)
    assert store.job["cancelled"] and overview.property("buttonDim") is True
    while store.job["ok"] is None:
        wait_for(store.jobChanged)
    pump(50)
    assert store.job["message"].startswith("Stopped after ") and overview.property("buttonLabel") == "Fetch missing art"
    window.close()
    pump(50)


def test_the_switch2_artwork_page_opens_a_slot_with_what_shows_first(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject

    api.theme.set("switch2")
    _engine, window = render(api, activate=True)
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


def test_b_held_opens_the_power_menu_in_both_looks(api):
    from PySide6.QtCore import QObject, Qt
    from PySide6.QtTest import QTest

    def hold(ms):
        QTest.keyPress(window, Qt.Key.Key_Escape)
        pump(ms)
        QTest.keyRelease(window, Qt.Key.Key_Escape)
        pump(50)

    wait_for(api.system.changed, 3000)
    _engine, window = render(api, activate=True)
    confirm = window.findChild(QObject, "confirm")
    hold(150)
    assert confirm.property("open") is False, "a tap is a tap"
    hold(600)
    assert confirm.property("open") is True and confirm.property("message") == "Power"
    assert [i["label"] for i in confirm.property("items").toVariant()] == ["Quit Universe", "Suspend", "Reboot", "Power off", "Stay"]
    assert confirm.property("index") == 0, "Quit Universe first, under the cursor"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    pump(100)
    assert confirm.property("open") is False, "B on the menu stays"
    api.theme.set("switch2")
    settle(window)
    picker = window.findChild(QObject, "picker")
    hold(600)
    assert picker.property("open") is True and picker.property("title") == "Power Options"
    assert picker.property("choices").toVariant() == ["Quit Universe", "Sleep Mode", "Restart", "Turn Off"]
    hold(600)
    assert picker.property("open") is False, "held on the menu: B closes it and the hold asks nothing more"
    dialog = window.findChild(QObject, "dialog")
    hold(600)
    for _ in range(3):
        QTest.keyClick(window, Qt.Key.Key_Down)
        pump(30)
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(100)
    assert dialog.property("open") is True and dialog.property("message") == "Turn off the system?"
    assert dialog.property("index") == 0, "Cancel under the cursor"
    QTest.keyClick(window, Qt.Key.Key_Right)
    QTest.keyClick(window, Qt.Key.Key_Return)
    for _ in range(100):
        if api.universe.core.powered:
            break
        pump(30)
    assert api.universe.core.powered == ["power_off"]
    window.close()
    pump(50)


def test_reboot_and_power_off_ask_again_and_close_the_game_first(api, fake, monkeypatch):
    from PySide6.QtCore import QObject, Qt
    from PySide6.QtTest import QTest

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)

    def pick(downs):
        root.askPower()
        pump(100)
        for _ in range(downs):
            QTest.keyClick(window, Qt.Key.Key_Down)
            pump(30)
        QTest.keyClick(window, Qt.Key.Key_Return)
        pump(100)

    def powered(n):
        for _ in range(100):
            if len(fake.core.powered) >= n:
                break
            pump(30)
        return fake.core.powered

    wait_for(api.system.changed, 3000)
    fake.launch("mirrors-edge", "")
    wait_for(fake.launched, 3000)
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    confirm = window.findChild(QObject, "confirm")
    pick(1)
    assert confirm.property("open") is False and powered(1) == ["suspend"]
    assert fake.core.current(), "suspend leaves the game running"
    pick(3)
    assert confirm.property("open") is True and confirm.property("message") == "Power off the computer?"
    assert confirm.property("note") == "Mirror's Edge is closed first."
    assert confirm.property("index") == 0, "Cancel under the cursor: one A too many powers nothing off"
    QTest.keyClick(window, Qt.Key.Key_Return)
    pump(200)
    assert confirm.property("open") is False and fake.core.powered == ["suspend"]
    pick(2)
    assert confirm.property("message") == "Reboot the computer?"
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(30)
    QTest.keyClick(window, Qt.Key.Key_Return)
    assert powered(2) == ["suspend", "reboot"]
    assert fake.core.current() is None, "the game is stopped before the reboot"
    fake.core.power_error = 'Operation inhibited by "nosleep"'
    api.system.run("suspend")
    assert wait_for(api.system.failed, 3000) == ("suspend", 'Operation inhibited by "nosleep"')
    window.close()
    pump(50)


def test_the_power_menu_lists_what_logind_would_do(fake):
    from universe_ui.api import System

    fake.core.power_list = ["reboot"]
    system = System(fake)
    wait_for(system.changed, 3000)
    assert system.actions == ["reboot"]


def test_reprise_about_shows_the_build(api, fake):
    _engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("settingsTab"))
    settle(window)
    page = root.property("activePage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "quit"))
    pump(100)
    assert page.property("sectionId") == "about", "Quit lives in About now"
    content = page.property("content").toVariant()
    assert [r["label"] for r in content["rows"]] == ["Version", "First-run setup", "Power"]
    assert content["rows"][0]["display"] == fake.version()
    assert page.property("acceptLabel") == "", "nothing to select on the version"
    assert [s["id"] for s in page.property("sections").toVariant()][-3:] == ["sound", "doctor", "about"], "no Search, Updates or Quit section"
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", 2), Q_ARG("QVariant", content["rows"][2]))
    pump(100)
    confirm = window.findChild(QObject, "confirm")
    assert confirm.property("open") is True and confirm.property("message") == "Power", "the row opens the menu B held opens"
    window.close()
    pump(50)


def test_the_reprise_game_menu_groups_its_rows_and_hides_the_media_a_game_has_none_of(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
            pump(80)

    def actions():
        return [i["action"] for i in menu.property("items").toVariant()]

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    menu = window.findChild(QObject, "gameMenu")
    click(Qt.Key.Key_F1)
    assert menu.property("open") is True and root.property("activePage").property("currentGame").id == "the-technomancer"
    items = menu.property("items").toVariant()
    assert [i["action"] for i in items] == ["play", "details", "favourite", "media", "manage"]
    assert [i.get("gap", False) for i in items] == [False, True, False, False, False], "play, then the rest"
    assert items[3]["more"] is True and items[4]["more"] is True
    click(Qt.Key.Key_Down, 3)
    click(Qt.Key.Key_Return)
    assert menu.property("open") is True and menu.property("title") == "Media" and len(menu.property("stack").toVariant()) == 1
    counts = {kind: len(getattr(fake, kind)("the-technomancer")) for kind in ("screenshots", "recordings", "journal")}
    assert all(counts.values()), "the fixture game has every kind"
    assert [i["action"] for i in menu.property("items").toVariant()] == list(counts), "the kinds the game has, no counts"
    click(Qt.Key.Key_Escape)
    assert menu.property("open") is True and menu.property("index") == 3 and actions()[3] == "media", "B comes back to the row that opened it"
    click(Qt.Key.Key_Escape)
    assert menu.property("open") is False

    page = root.property("activePage")
    game = api.allGames.byId("mini-metro")
    QMetaObject.invokeMethod(root, "openMenu", Q_ARG("QVariant", game), Q_ARG("QVariant", page.property("menuAnchor")))
    pump(100)
    assert actions() == ["play", "details", "favourite", "manage"], "nothing to browse: no Media row"
    click(Qt.Key.Key_Down, 3)
    click(Qt.Key.Key_Return)
    assert menu.property("title") == "Manage" and actions() == ["settings", "artwork", "sessions", "remove"]
    assert menu.property("items").toVariant()[3]["danger"] is True
    click(Qt.Key.Key_Down, 3)
    click(Qt.Key.Key_Return)
    assert menu.property("open") is True and menu.property("title") == "Remove Mini Metro?"
    click(Qt.Key.Key_Down)
    click(Qt.Key.Key_Return)
    assert menu.property("open") is False
    assert wait_for(fake.libraryChanged, 3000) == (["mini-metro"],)
    pump(100)
    assert api.allGames.byId("mini-metro") is None
    window.close()
    pump(50)


def test_the_game_settings_page_lands_a_search_hit_behind_advanced(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(80)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/GameSettingsPage.qml", {"game": game, "key": "launch.ntsync"})
    settle(window)
    pump(300)
    page = window.findChild(QObject, "gameSettingsPage")
    body = page.findChild(QObject, "cardSections")
    form = api.screens.gameSettings
    assert page is not None and form.showAdvanced is True, "an advanced row: Advanced comes on"
    sections = [s["name"] for s in page.property("sections").toVariant()]
    assert sections == ["Display", "Overlay", "Proton", "Launch", "Desktop and library", "Video capture", "Play journal", "Screenshots"], (
        "no advanced card of its own"
    )
    assert sections[body.property("section")] == "Proton" and page.property("row")["key"] == "launch.ntsync", (
        "the hit sits in the Proton card, the cursor on it"
    )
    assert [h["label"] for h in page.property("hints").toVariant()] == ["Toggle", "More", "Back"]
    assert page.property("canReset") is False, "nothing of the game's to drop"
    assert [i["action"] for i in page.property("moreItems").toVariant()] == ["advanced"], "More lists what X and Y do here"
    click(Qt.Key.Key_Return)
    assert fake.game("the-technomancer")["launch"]["ntsync"] is False and page.property("row")["origin"] == "game", (
        "changing the value is what sets it on the game"
    )
    assert page.property("canReset") is True
    click(Qt.Key.Key_F1)
    menu = next(c for c in page.findChildren(QObject) if c.property("stack") is not None and c.property("open"))
    assert menu.property("open") is True and [i["action"] for i in menu.property("items").toVariant()] == ["reset", "advanced"]
    click(Qt.Key.Key_Escape)
    assert menu.property("open") is False
    click(Qt.Key.Key_I)
    assert "ntsync" not in fake.game("the-technomancer")["launch"] and page.property("row")["origin"] == "default", "X drops it"
    click(Qt.Key.Key_F)
    assert form.showAdvanced is False and sections == [s["name"] for s in page.property("sections").toVariant()], "Y: the rows go, the sidebar stays"
    assert form.groups[body.property("section")]["title"] == "Proton" and page.property("row")["key"] == "launch.proton", (
        "the cursor lands on the card's first row"
    )
    click(Qt.Key.Key_Escape)
    assert [h["label"] for h in page.property("hints").toVariant()] == ["Open", "Back"], "B: the sidebar"
    click(Qt.Key.Key_PageUp)
    assert form.groups[body.property("section")]["title"] == "Overlay", "LT steps the card"
    click(Qt.Key.Key_Escape)
    assert root.property("subOpen") is False, "B from the sidebar closes the page"
    root.openSub("pages/GameSettingsPage.qml", {"game": game})
    settle(window)
    pump(300)
    assert form.showAdvanced is False, "reopened, Advanced starts hidden"
    window.close()
    pump(50)


def test_the_game_settings_page_adds_a_variable_from_one_sheet(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(80)

    def type_text(text):
        for ch in text:
            QTest.keyClick(window, ch)
        pump(80)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/GameSettingsPage.qml", {"game": game, "key": "launch.env"})
    settle(window)
    pump(300)
    page = window.findChild(QObject, "gameSettingsPage")
    body = page.findChild(QObject, "cardSections")
    form = api.screens.gameSettings
    row = page.property("row")
    assert form.groups[body.property("section")]["title"] == "Launch" and row["map"] is True and row["label"] == "Add a variable…", (
        "the environment folds into Launch; the hit lands on the row that adds a variable"
    )
    assert [h["label"] for h in page.property("hints").toVariant()][:2] == ["Add", "More"]
    click(Qt.Key.Key_Return)
    assert [h["label"] for h in page.property("hints").toVariant()] == ["Next", "Cancel"], "one sheet, two fields: the name first"
    type_text("DXVK_HUD")
    click(Qt.Key.Key_Return)
    assert [h["label"] for h in page.property("hints").toVariant()] == ["Save", "Cancel"], "then the value"
    type_text("fps")
    click(Qt.Key.Key_Return)
    pump(200)
    assert fake.game("the-technomancer")["launch"]["env"] == {"DXVK_HUD": "fps"}
    row = page.property("row")
    assert row["key"] == "launch.env.DXVK_HUD" and row["origin"] == "game", "the new variable is a row of its own, the cursor on it"
    assert [h["label"] for h in page.property("hints").toVariant()][:2] == ["Change", "More"]
    assert page.property("moreItems").toVariant()[0]["label"] == "Remove", "More lists the variable's removal"
    click(Qt.Key.Key_I)
    assert fake.game("the-technomancer")["launch"].get("env", {}) == {}, "X removes it"
    window.close()
    pump(50)


def test_a_path_row_is_typed_first_under_a_keyboard(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    from universe_ui import gamepad

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(80)

    def labels():
        return [h["label"] for h in page.property("hints").toVariant()]

    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/GameSettingsPage.qml", {"game": game, "key": "launch.exe"})
    settle(window)
    pump(300)
    page = window.findChild(QObject, "gameSettingsPage")
    assert page.property("row")["key"] == "launch.exe" and page.property("row")["type"] == "path"
    gamepad.post_key(Qt.Key.Key_Return, True, window=window)
    gamepad.post_key(Qt.Key.Key_Return, False, window=window)
    pump(150)
    assert api.keys.mode == "pad" and labels() == ["Up", "More", "Cancel"], "under a pad the folders come first"
    click(Qt.Key.Key_Escape)
    click(Qt.Key.Key_Return)
    assert api.keys.mode == "keyboard" and labels() == ["Done", "Browse", "Cancel"], "under a keyboard the path is typed first"
    click("2")
    click(Qt.Key.Key_Return)
    pump(200)
    assert fake.game("the-technomancer")["launch"]["exe"] == "/mnt/games/PC/The Technomancer/TheTechnomancer.exe2", "the field held the value"
    click(Qt.Key.Key_Return)
    click(Qt.Key.Key_F1)
    assert labels() == ["Up", "More", "Cancel"] and api.screens.paths.files is True, "F1: the folders, for a file"
    click(Qt.Key.Key_F)
    assert labels() == ["Done", "Browse", "Cancel"], "Y: back to typing"
    click(Qt.Key.Key_Escape)
    assert labels()[0] == "Change" and warnings == []
    window.close()
    pump(50)


def test_the_sheets_type_the_physical_keyboards_letters(fake, tmp_path, monkeypatch):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    from universe_ui import gamepad
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(80)

    def press(key=Qt.Key.Key_Return):
        gamepad.post_key(key, True, window=window)
        gamepad.post_key(key, False, window=window)
        pump(150)

    monkeypatch.setenv("XKB_DEFAULT_LAYOUT", "fr")
    monkeypatch.setenv("XKB_DEFAULT_VARIANT", "")
    exe = tmp_path / "The Technomancer" / "TheTechnomancer.exe"
    exe.parent.mkdir()
    fake.set("the-technomancer", "launch.exe", str(exe))
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/GameSettingsPage.qml", {"game": game, "key": "launch.exe"})
    settle(window)
    pump(300)
    page = window.findChild(QObject, "gameSettingsPage")
    press()
    click(Qt.Key.Key_F)
    assert "Browse" in [h["label"] for h in page.property("hints").toVariant()], "Y on the folders: the keyboard sheet"
    press()
    click(Qt.Key.Key_Down, 4)
    press()
    click(Qt.Key.Key_Up, 4)
    press()
    press(Qt.Key.Key_F)
    pump(200)
    assert fake.game("the-technomancer")["launch"]["exe"] == os.path.dirname(exe) + "a&", (
        "the first letter key is A, and ⇧ on the number row types the layout's own level"
    )

    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsSearchPage.qml"), Q_ARG("QVariant", {}))
    settle(window)
    pump(300)
    press()
    click(Qt.Key.Key_Down, 3)
    press()
    click(Qt.Key.Key_Up, 4)
    press()
    press()
    assert api.screens.search.query == "a&1", "the Switch's ⇧ types one key"
    window.close()
    pump(50)
    api.shutdown()


def test_the_switch2_forms_share_the_sidebar_and_y(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
        pump(80)

    def push(source, args):
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
        settle(window)
        pump(300)
        return root.property("topPage")

    def labels(page):
        return [h["label"] for h in page.property("hints").toVariant()]

    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    form = api.screens.runner
    page = push("pages/FormPage.qml", {"runner": "proton"})
    sections = [s["label"] for s in page.property("sections").toVariant()]
    assert sections == ["Runner", "Proton", "Games"] and form.showAdvanced is False
    click(Qt.Key.Key_F)
    assert form.showAdvanced is True and [s["label"] for s in page.property("sections").toVariant()] == sections, "Y: the sidebar stays"
    click(Qt.Key.Key_Right)
    click(Qt.Key.Key_Down, 2)
    row = page.property("currentRow").toVariant()
    assert row["key"] == "gamescope" and row["origin"] == "global" and labels(page) == ["Hide advanced", "Reset", "Back", "Toggle"]
    click(Qt.Key.Key_Return)
    assert form.rows[row["form"]]["origin"] == "runner", "toggling the inherited switch sets it on the runner"
    click(Qt.Key.Key_I)
    assert form.rows[row["form"]]["origin"] == "global", "X clears it back"
    click(Qt.Key.Key_Escape, 2)
    page = push("pages/FormPage.qml", {"source": "gog"})
    pump(500)
    assert [s["label"] for s in page.property("sections").toVariant()] == ["Settings", "Sign-in"] and labels(page) == ["Show advanced", "Back", "OK"]
    click(Qt.Key.Key_Escape)
    page = push("pages/SettingsPage.qml", {"section": "launch"})
    click(Qt.Key.Key_F)
    assert api.screens.launch.showAdvanced is True, "Y opens the Launch section's advanced rows"
    assert warnings == []
    window.close()
    pump(50)


# The fixture's pad supply hangs off event30: the fake pad's node, so its glyph goes green while that pad is current.
def test_the_badge_tints_the_current_pad(api):
    from PySide6.QtGui import QColor

    from universe_ui.screens.controller import FakeWatcher

    _engine, window = render(api)
    controller = api.screens.controller
    controller.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    controller.start(watcher)
    pump(50)
    badge = next(c for c in window.findChildren(QObject) if c.property("currentTint") is not None)
    sources = {c.property("current"): c for c in badge.childItems() if c.property("low") is not None}
    assert set(sources) == {True, False}, "the laptop and the pad"
    assert sources[True].property("ink") == badge.property("currentTint") and sources[False].property("ink") == badge.property("tint")
    watcher.emit({"event": "gone", "id": "event30"})
    pump(50)
    pad = sources[True]
    assert pad.property("current") is False and pad.property("ink") == QColor(badge.property("tint")), "no pad current, no green"
    window.close()
    pump(50)


def test_the_sound_section_plays_through_the_output_picked_in_both_looks(api, fake):
    from PySide6.QtTest import QTest

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("settingsTab"))
    settle(window)
    page = root.property("activePage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "sound"))
    wait_for(api.home.outputsChanged, 3000)
    pump(100)
    rows = page.property("content").toVariant()["rows"]
    assert [(r["label"], r["display"], r["tag"]) for r in rows] == [
        ("Speakers", "Built-in Audio", "In use"),
        ("Headphones", "Built-in Audio", ""),
        ("HDMI / DisplayPort", "TV", ""),
    ]
    assert page.property("acceptLabel") == "", "the output in use: nothing to do"
    QTest.keyClick(window, Qt.Key.Key_Down)
    pump(80)
    assert page.property("acceptLabel") == "Use"
    QTest.keyClick(window, Qt.Key.Key_Return)
    wait_for(api.home.outputsChanged, 3000)
    pump(80)
    assert [r["label"] for r in page.property("content").toVariant()["rows"] if r["tag"]] == ["Headphones"]
    window.close()
    pump(50)

    api.theme.set("switch2")
    _engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "sound"}))
    settle(window)
    page = root.property("topPage")
    rows = page.property("content").toVariant()
    assert [(r["label"], r["path"], r["value"]) for r in rows] == [
        ("Speakers", "Built-in Audio", False),
        ("Headphones", "Built-in Audio", True),
        ("HDMI / DisplayPort", "TV", False),
    ]
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", 2), Q_ARG("QVariant", rows[2]))
    wait_for(api.home.outputsChanged, 3000)
    assert [r["label"] for r in page.property("content").toVariant() if r["value"]] == ["HDMI / DisplayPort"]
    window.close()
    pump(50)
