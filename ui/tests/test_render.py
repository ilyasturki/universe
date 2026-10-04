import os

import pytest
from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QObject, Qt, QUrl
from PySide6.QtGui import QColor
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuick import QQuickItem, QQuickWindow  # noqa: F401  (QQuickWindow: rootObjects() down-cast, for grabWindow)

from conftest import pump, record, until
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
    """Until the window draws the scene as it stands; an animation may still be running."""
    drawn = []

    def swapped():
        drawn.append(True)

    window.frameSwapped.connect(swapped)
    window.update()
    until(lambda: drawn, "the window drew no frame")
    window.frameSwapped.disconnect(swapped)


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


def js(obj, name):
    value = obj.property(name)
    return value.toVariant() if hasattr(value, "toVariant") else value


def page_as(root, kind, slot="activePage"):
    """`root`'s page in `slot` once it is a `kind` (HomePage, SettingsPage…): Reprise loads a tab's page asynchronously."""
    return until(lambda: (p := root.property(slot)) is not None and p.metaObject().className().split("_QML")[0] == kind and p, f"no {kind} as {slot}")


def game_id(page):
    game = page.property("currentGame")
    return game.property("id") if game is not None else None


def lit(window, ground, above):
    until(lambda: lit_fraction(window.grabWindow(), ground) > above, f"less than {above:.0%} of the window drawn")


def test_the_scene_holds_still_behind_the_game(api, fake, monkeypatch):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    _engine, window = render(api)
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    until(lambda: count_frames(window, 200) == 0, "the scene goes still under the game")
    covered = count_frames(window, 600)
    api.home.toLauncher()
    until(lambda: api.home.shown == "launcher")
    shown = count_frames(window, 600)
    assert covered <= 3 < shown, f"{covered} frames under the game, {shown} with the launcher up: the badge pulses and the hero drifts only when seen"


def test_themes_render_and_switch_live(api):
    _engine, window = render(api)
    image = window.grabWindow()
    assert image.width() == 1280 and image.height() == 720
    lit(window, api.theme.ground, 0.05)
    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    top = until(lambda: root.property("depth") == 1 and root.property("topPage"), "a switch lands on the new look's Themes page")
    assert top.property("sectionId") == "themes", "a switch lands on the new look's Themes page"
    lit(window, api.theme.ground, 0.01)
    assert window.grabWindow().pixelColor(4, 4).name() == api.theme.ground
    assert api.theme.landing == "", "taken once"
    api.theme.set("reprise")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    page = page_as(root, "SettingsPage")
    assert root.property("tabIndex") == root.property("settingsTab")
    until(lambda: page.property("sectionId") == "themes")
    lit(window, api.theme.ground, 0.01)
    settle(window)
    window.close()


def test_the_ps5_look_renders_and_lands_on_its_themes(api):
    _engine, window = render(api)
    api.theme.set("ps5")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    top = until(lambda: root.property("depth") == 1 and root.property("topPage"), "a switch lands on the new look's Settings")
    assert top.property("sectionId") == "themes" and top.property("level") == "section", "open on the Themes section"
    lit(window, api.theme.ground, 0.01)
    assert api.theme.landing == "", "taken once"
    api.theme.set("reprise")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    until(lambda: page_as(root, "SettingsPage").property("sectionId") == "themes")
    settle(window)
    window.close()


@pytest.mark.parametrize("skip", [True, False], ids=["skipped", "played"])
@pytest.mark.parametrize("look", ["reprise", "ps5", "switch2"])
def test_the_intro_plays_then_leaves_home_the_focus_and_no_press(fake, tmp_path, look, skip):
    from PySide6.QtCore import QObject
    from PySide6.QtTest import QTest

    from universe_ui.api import Api
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE

    def has_focus(item):
        at = window.activeFocusItem()
        while at is not None and at != item:
            at = at.parentItem()
        return at is not None

    api = Api(fake, memory_path=str(tmp_path / "memory.json"), theme=look, power_root=FAKE, net_root=FAKE_NET, boot=True)
    try:
        _engine, window = render(api, activate=True)
        root = window.property("contentItem").childItems()[0].property("item")
        boot = window.findChild(QObject, "boot")
        home = page_as(root, "HomePage") if look == "reprise" else until(lambda: root.findChild(QObject, "homePage"))
        start = until(lambda: game_id(home))
        assert api.boot.running and boot.property("visible")
        until(lambda: boot.property("ring") > 0, "the mark comes in")
        if skip:
            QTest.keyClick(window, Qt.Key.Key_Right)
            until(lambda: not api.boot.running, "a press skips it", 1000)
        else:
            until(lambda: not api.boot.running, "it ends by itself", 3000)
        assert game_id(home) == start, "the press that skipped stays the intro's"
        until(lambda: has_focus(home), "home holds the focus")
        QTest.keyClick(window, Qt.Key.Key_Right)
        until(lambda: game_id(home) != start, "the next press moves home")
        assert not boot.property("visible")
        api.theme.set("ps5" if look != "ps5" else "reprise")
        settle(window)
        assert not api.boot.running and not boot.property("visible"), "a switch does not replay it"
        window.close()
    finally:
        api.shutdown()


def test_the_media_tab_and_the_screenshots_page(api, fake):
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(1)
    page = page_as(root, "MediaPage")

    def read(name):
        value = page.property(name)
        return value.toVariant() if hasattr(value, "toVariant") else value

    def rows():
        return read("rows")

    def current():
        return read("current")

    until(lambda: not api.screens.media.loading and rows())
    assert root.property("tabIndex") == 1 and current()["kind"] in ("shot", "recording", "journal")
    assert {r["kind"] for r in rows()} == {"shot", "recording", "journal"}, "one grid, every kind, no filter"
    assert [h["glyph"] for h in read("hints")] == ["A", "Start", "B"]
    lit(window, api.theme.ground, 0.05)
    page.setProperty("index", next(i for i, r in enumerate(rows()) if r["kind"] == "shot" and r["gameId"] == "the-technomancer"))
    page.open()
    until(lambda: page.property("lightbox") is True and page.property("modal") is True)
    page.setProperty("lightbox", False)
    game = page.property("currentGame")
    assert game is not None and game.property("id") == "the-technomancer"
    root.openSub("pages/ScreenshotsPage.qml", {"game": game, "name": current()["name"]})
    assert root.property("subOpen") is True and root.property("subSource") == "pages/ScreenshotsPage.qml"
    shots = api.screens.shots
    until(lambda: shots.gameId == "the-technomancer" and shots.count > 0, "the sub-page loaded the game's shots")
    lit(window, api.theme.ground, 0.05)
    root.closeSub()
    until(lambda: root.property("subOpen") is False)
    settle(window)
    window.close()


def test_the_tab_bar_search_finds_the_settings_under_the_games(api, fake):
    from PySide6.QtTest import QTest

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.openSearch()
    overlay = page_as(root, "SearchOverlay", "focusTarget")
    search = api.screens.search
    until(lambda: search.ready)
    assert [s["id"] for s in search.sections][:3] == ["launch", "runners", "controller"], "indexed with Reprise's sections before Settings ever opened"
    overlay.setProperty("query", "techno")
    until(lambda: overlay.property("hasGames") is True and overlay.property("hasSettings") is False, "a title alone: the cover, not the game's every setting")
    overlay.setProperty("query", "quit")
    until(
        lambda: {"page": "section", "id": "about", "key": "", "module": ""} in [r["target"] for r in search.results],
        "Quit lives in About: its word finds About",
    )
    overlay.setProperty("query", "vrr")
    until(lambda: overlay.property("hasSettings") is True and overlay.property("hasGames") is False)
    overlay.toResults()
    until(lambda: overlay.property("zone") == "settings")
    assert [h["label"] for h in overlay.property("hints").toVariant()] == ["Open", "Close"]
    QTest.keyClick(window, Qt.Key.Key_Return)
    page = page_as(root, "SettingsPage")
    assert root.property("searchOpen") is False and root.property("tabIndex") == root.property("settingsTab")

    def row():
        cards = next((c for c in page.findChildren(QObject) if c.property("focusRect") is not None), None)
        return (cards.property("currentRow") or {}).get("label") if cards is not None else None

    until(lambda: page.property("sectionId") == "launch" and row() == "Adaptive sync", "the hit's row, on its section")
    settle(window)
    window.close()


def test_the_screenshots_page_puts_the_running_sessions_shots_first(api, fake):
    from PySide6.QtCore import QObject

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    root.openSub("pages/ScreenshotsPage.qml", {"game": game})
    page = until(lambda: window.findChild(QObject, "screenshotsPage"), "the screenshots page is up")
    earlier = until(lambda: page.property("rows").toVariant())
    assert page.property("since") == "" and page.property("mine") == 0, "no session: one run, no headings"
    grid = next(c for c in page.findChildren(QObject) if c.property("cellHeight") is not None)
    card = until(lambda: QMetaObject.invokeMethod(grid, "currentCard", Qt.DirectConnection, Q_RETURN_ARG("QVariant")))
    picture = card.property("height") - card.property("captionHeight")
    assert abs(picture - card.property("width") * 9 / 16) < 1, "the cell makes room for the date line: the picture stays 16:9, whole"
    fake.launch("the-technomancer", "")
    until(lambda: api.universe.currentSession)
    api.home.screenshot()
    until(lambda: page.property("mine") == 1, "the playing game's page splits at the session's start")
    assert page.property("since") != ""
    rows = page.property("rows").toVariant()
    assert len(rows) == len(earlier) + 1 and rows[0]["name"] not in {r["name"] for r in earlier}, "the new shot leads"
    api.universe.stop(api.universe.currentSession["session_id"])
    until(lambda: page.property("since") == "", "the session over, one run again")
    settle(window)
    window.close()


def test_the_switch2_album_holds_the_shots_too(api):
    from PySide6.QtCore import Q_ARG, QMetaObject

    api.theme.set("switch2")
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/AlbumPage.qml"), Q_ARG("QVariant", {}))
    top = page_as(root, "AlbumPage", "topPage")
    shown = until(lambda: top.property("shown").toVariant())
    kinds = {r["kind"] for r in shown}
    assert kinds == {"shot", "recording"} and [r["when"] for r in shown] == sorted((r["when"] for r in shown), reverse=True)
    top.setProperty("kindFilter", "shot")
    until(lambda: (shown := top.property("shown").toVariant()) and all(r["kind"] == "shot" for r in shown))
    top.play()
    until(lambda: top.property("viewing") is True)
    lit(window, api.theme.ground, 0.05)
    settle(window)
    window.close()


def test_a_session_running_at_startup_is_home_with_the_game_pinned(api, fake):
    from PySide6.QtCore import QObject

    fake.launch("mirrors-edge", "")
    until(lambda: fake.currentSession)
    _engine, window = render(api, activate=True)
    overlay = window.findChild(QObject, "launchOverlay")
    assert overlay is not None
    assert overlay.property("running") is False
    root = window.property("contentItem").childItems()[0].property("item")
    assert root.property("playingId") == "mirrors-edge"
    home = page_as(root, "HomePage")
    until(lambda: game_id(home) == "mirrors-edge")
    assert home.property("playLabel") == "Resume"
    fake.core.end_session()
    until(lambda: root.property("playingId") == "")
    assert fake.currentSession is None
    settle(window)
    window.close()


def test_the_cursor_follows_the_game_through_its_session(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = page_as(root, "HomePage")
    overlay = window.findChild(QObject, "launchOverlay")
    until(lambda: game_id(home) == "the-technomancer")
    for ident in ("dead-cells", "mirrors-edge"):  # a row on the rail, then a first play that has none yet
        until(lambda: not overlay.property("running"))
        before = game_id(home)
        QMetaObject.invokeMethod(root, "launchGame", Q_ARG("QVariant", api.allGames.byId(ident)))
        until(lambda: root.property("playingId") == ident)  # noqa: B023
        assert game_id(home) == before
        fake.stop("")
        until(lambda: root.property("playingId") == "")
        until(lambda: game_id(home) == ident)  # noqa: B023
    settle(window)
    window.close()


def test_the_switch2_home_row_follows_the_game_too(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    api.theme.set("switch2")
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = until(lambda: root.findChild(QObject, "homePage"))
    until(lambda: game_id(home) == "the-technomancer")
    QMetaObject.invokeMethod(root, "launch", Q_ARG("QVariant", api.allGames.byId("dead-cells")), Q_ARG("QVariant", None))
    until(lambda: fake.currentSession and home.property("index") == 1, "the played game moves to the front")
    assert game_id(home) == "the-technomancer", "the cursor stays on its game"
    fake.stop("")
    until(lambda: game_id(home) == "dead-cells" and home.property("index") == 0)
    settle(window)
    window.close()


def test_a_launch_holds_the_poster_until_the_window_is_shown(api, fake, monkeypatch):
    import threading

    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    overlay = window.findChild(QObject, "launchOverlay")
    mapped, shown = threading.Event(), record(fake.sessionShown)
    wait_session_window = fake.core.wait_session_window
    monkeypatch.setattr(fake.core, "wait_session_window", lambda *args: (mapped.wait(5), wait_session_window(*args))[1])
    QMetaObject.invokeMethod(root, "launchGame", Q_ARG("QVariant", api.allGames.byId("control")))
    assert overlay.property("running") is True and root.property("launching") is True
    until(lambda: fake.currentSession)
    until(lambda: overlay.property("waiting") is True)
    assert fake.currentSession["id"] == "control"
    assert fake.core.last_splash.endswith("splash-control.bgrx")
    with open(fake.core.last_splash, "rb") as f:
        header = f.readline().decode().split()
        assert [int(v) for v in header] == [round(window.width() * window.devicePixelRatio()), round(window.height() * window.devicePixelRatio())]
        assert len(f.read()) == int(header[0]) * int(header[1]) * 4
    mapped.set()
    assert until(lambda: shown)[0][1] is True
    until(lambda: overlay.property("running") is False)
    assert root.property("launching") is False
    assert root.property("playingId") == "control"
    settle(window)
    window.close()


def test_the_overlay_draws_the_volume_level_inside_gamescope(api, fake, monkeypatch):
    from PySide6.QtCore import QRect, QSize

    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    monkeypatch.setattr(fake, "overlay", lambda window, input, opacity: None)
    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("api", api)
    overlay = host.create_overlay(engine, QSize(1280, 720))
    assert api.home.attachOverlay(overlay)
    settle(overlay)
    band = QRect(320, 540, 640, 120)
    before = lit_fraction(overlay.grabWindow().copy(band), "#000000")
    api.screens.controller._on_event({"event": "volume", "percent": 60, "muted": False, "output": "Speakers"})
    until(lambda: lit_fraction(overlay.grabWindow().copy(band), "#000000") > before + 0.05, "the level's pill above the bottom edge")
    shot = overlay.grabWindow()
    if os.environ.get("UNIVERSE_SHOT_DIR"):
        shot.save(os.path.join(os.environ["UNIVERSE_SHOT_DIR"], "overlay-osd.png"))
    assert api.home.osd and lit_fraction(shot.copy(band), "#000000") > before + 0.05, "the level's pill above the bottom edge"


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


def test_a_second_store_switches_the_install_pages_of_both_looks(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject

    fake.core._source("epic").update(enabled=True, logged_in=True)
    fake.core._data["source_library"]["epic"] = [{"id": "Min", "title": "Hades", "owned": True, "installed": False}]
    sources = api.screens.sources
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")

    def store_card():
        content = js(page, "content")
        return content["rows"][content["groups"][0]["rows"][0]]["display"] if content and content["groups"] else None

    root.setProperty("tabIndex", root.property("settingsTab"))
    page = page_as(root, "SettingsPage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "install"))
    until(lambda: sources.rows and store_card() == "GOG", "the store card comes first")
    assert js(page, "content")["groups"][0]["title"] == "Store"
    sources.pick("epic")
    until(lambda: [r["title"] for r in sources.rows] == ["Hades"] and store_card() == "Epic Games")
    assert [r["label"] for r in js(page, "content")["rows"] if r["key"] == "game"] == ["Hades"]

    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/InstallPage.qml"), Q_ARG("QVariant", {}))
    install = page_as(root, "InstallPage", "topPage")
    until(lambda: install.property("sourceName") == "Epic Games" and [c["game"]["title"] for c in js(install, "cells") if not c.get("heading")] == ["Hades"])
    settle(window)
    window.close()


def test_the_install_pages_render_a_running_install_in_both_looks(api, fake, monkeypatch):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "STEP_S", 0.15)

    sources = api.screens.sources
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")

    finished = record(fake.jobFinished)

    def meta():
        content = js(page, "content")
        return content["groups"][0]["meta"] if content and content["groups"] else None

    root.setProperty("tabIndex", root.property("settingsTab"))
    page = page_as(root, "SettingsPage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "install"))
    until(
        lambda: [g["title"] for g in js(page, "content")["groups"]] == ["Installing", "Updates", "Installed", "Owned, not installed"],
        "the pending updates fold into Install",
    )
    content = js(page, "content")
    groups = content["groups"]
    assert [content["rows"][i]["label"] for i in groups[0]["rows"]] == ["Disco Elysium"], "the paused download sits in the Installing card"
    assert [content["rows"][i]["label"] for i in groups[1]["rows"]] == ["Update everything"]
    assert groups[0]["meta"] == "1 paused" and " GB · /mnt/games/PC" in groups[2]["meta"]
    row = next(i for i, r in enumerate(sources.rows) if r["title"] == "Stardew Valley")
    sources.install(row)
    until(lambda: meta() == "1 running · 1 paused")
    assert sources.cancel()
    until(lambda: finished)
    until(lambda: meta() == "2 paused")

    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/InstallPage.qml"), Q_ARG("QVariant", {}))
    install = page_as(root, "InstallPage", "topPage")
    until(lambda: [c["label"] for c in js(install, "cells") if c.get("heading")] == ["Owned, not installed"])
    assert [line["label"] for line in js(install, "lines") if line.get("heading")] == ["Installing", "Installed"]
    assert [h["glyph"] for h in js(install, "hints")] == ["Y", "B", "A"]
    QTest.keyClick(window, Qt.Key.Key_E)  # RB: Manage
    until(lambda: install.property("tab") == 1)
    sources.install(row)
    until(lambda: "X" in [h["glyph"] for h in js(install, "hints")], "X cancels while a job runs")
    other = next(i for i, r in enumerate(sources.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    said = []
    sources.message.connect(said.append)
    assert sources.install(other) == "" and said == ["Installing Stardew Valley first — cancel it or wait"]
    assert sources.job["game"] == sources.rows[row]["id"], "one job at a time"
    QTest.keyClick(window, Qt.Key.Key_I)
    until(lambda: len(finished) == 2 and sources.job["cancelled"] and next(r for r in sources.rows if r["title"] == "Stardew Valley")["partial"])
    settle(window)
    window.close()


def test_the_component_bar_hides_in_both_looks_and_the_job_goes_on(api, fake):
    from PySide6.QtCore import QPoint, QPointF
    from PySide6.QtQuick import QQuickItem
    from PySide6.QtTest import QTest

    from universe_ui import gamepad

    form = api.screens.components
    finished = record(fake.jobFinished)
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")

    def js(obj, name):
        value = obj.property(name)
        return value.toVariant() if hasattr(value, "toVariant") else value

    def install(ident):
        until(form.listing)
        assert form.act(ident, "install")
        until(lambda: page.property("componentsBar") is True)
        settle(window)

    def hidden(page):
        return form.job is None and page.property("componentsBar") is False

    def centre(item):
        p = item.mapToScene(QPointF(item.width() / 2, item.height() / 2))
        return p.x(), p.y()

    root.setProperty("tabIndex", root.property("settingsTab"))
    page = page_as(root, "SettingsPage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "components"))
    until(lambda: page.property("sectionId") == "runners", "a landing on Components opens Runners")
    install("wine")
    assert "Hide progress" in [i["label"] for i in js(page, "moreItems")]
    QTest.keyClick(window, Qt.Key.Key_F)  # Y
    until(lambda: hidden(page), "Y hides it")
    until(lambda: len(finished) == 1)
    install("umu-run")
    x, y = centre(until(lambda: page.findChild(QQuickItem, "hideJob")))
    QTest.mouseClick(window, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier, QPoint(int(x), int(y)))
    until(lambda: hidden(page), "so does its ×")
    until(lambda: len(finished) == 2)

    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "components"}))
    page = page_as(root, "SettingsPage", "topPage")
    install("rpcs3")
    until(lambda: {"glyph": "X", "label": "Hide progress"} in js(page, "hints"))
    QTest.keyClick(window, Qt.Key.Key_I)  # X
    until(lambda: hidden(page), "X hides it")
    until(lambda: len(finished) == 3)
    install("proton-cachyos")
    gamepad.touch(window, [centre(until(lambda: page.findChild(QQuickItem, "hideJob")))], 0)
    until(lambda: hidden(page), "so does a tap on its ×")
    until(lambda: len(finished) == 4)
    QMetaObject.invokeMethod(root, "componentAction", Q_ARG("QVariant", "xemu"), Q_ARG("QVariant", "uninstall"))
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("message") == "Uninstall xemu?", "the uninstall asks first")
    assert dialog.property("dangerIndex") == 1, "Uninstall is the red button"
    settle(window)
    window.close()


def test_the_reprise_library_leads_with_hearts_and_y_hearts_the_game_under_the_cursor(api):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def title():
        game = page.property("currentGame")
        return game.property("title") if game is not None else None

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("libraryTab"))
    page = page_as(root, "LibraryPage")
    until(lambda: title() == "Dead Cells", "the hearted games first, by title")
    click(Qt.Key.Key_Right, 2)
    until(lambda: title() == "Batman: Arkham Origins", "then the rest, by title")
    assert [h["glyph"] for h in page.property("hints").toVariant()] == ["A", "Start", "B"]
    click(Qt.Key.Key_F)
    until(lambda: api.allGames.byId("batman-arkham-origins").favorite is True)
    until(lambda: title() == "Batman: Arkham Origins", "the cursor follows the game to its place among the hearts")
    click(Qt.Key.Key_Right)
    until(lambda: title() == "Dead Cells")
    settle(window)
    window.close()


def test_the_right_stick_pages_the_grids_in_both_looks(api):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    def title(page):
        game = page.property("currentGame")
        return game.property("title") if game is not None else None

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("libraryTab"))
    page = page_as(root, "LibraryPage")
    until(lambda: title(page))
    # Eight games on eight columns: the add tile alone on the second row.
    click(Qt.Key.Key_BracketLeft)
    first = title(page)
    assert first and page.property("onAddTile") is False, "a screenful up lands on the first row"
    click(Qt.Key.Key_BracketLeft)
    assert title(page) == first, "the first row is a clamp"
    click(Qt.Key.Key_Right, 3)
    click(Qt.Key.Key_BracketRight)
    until(lambda: page.property("onAddTile") is True, "down past the last game's row lands on the last cell")
    click(Qt.Key.Key_BracketRight, 3)
    assert page.property("onAddTile") is True
    click(Qt.Key.Key_BracketLeft)
    until(lambda: title(page) == first, "back up from the last cell: its own column, the first")
    settle(window)
    window.close()

    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/AlbumPage.qml"), Q_ARG("QVariant", {}))
    top = page_as(root, "AlbumPage", "topPage")
    first = until(lambda: top.property("current").toVariant())
    click(Qt.Key.Key_BracketRight)
    until(lambda: top.property("current").toVariant() != first)
    click(Qt.Key.Key_BracketLeft)
    until(lambda: top.property("current").toVariant() == first)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    for source, args, keys in (("pages/SettingsPage.qml", {"section": "launch"}, (Qt.Key.Key_Right,)), ("pages/AllSoftwarePage.qml", {}, ())):
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
        pushed = root.property("depth")
        page_as(root, source.removeprefix("pages/").removesuffix(".qml"), "topPage")
        settle(window)
        for key in (*keys, Qt.Key.Key_BracketRight, Qt.Key.Key_BracketRight, Qt.Key.Key_BracketLeft, Qt.Key.Key_Left, Qt.Key.Key_BracketRight):
            click(key)
        click(Qt.Key.Key_Escape, 2)
        until(lambda: root.property("depth") < pushed)  # noqa: B023
    assert warnings == []
    settle(window)
    window.close()


def test_the_artwork_page_opens_on_a_slot_and_lists_its_candidates(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    QMetaObject.invokeMethod(root, "openSub", Q_ARG("QVariant", "pages/ArtworkPage.qml"), Q_ARG("QVariant", {"game": game, "slot": "logo"}))
    form = api.screens.artwork
    assert root.property("subOpen") is True
    until(lambda: form.gameId == "the-technomancer")
    page = until(lambda: root.findChild(QObject, "artworkPage"))
    assert page.property("level") == "browser", "opened on a slot, the page went straight to its candidates"
    until(lambda: form.candidatesSlot == "logo" and form.candidates)
    lit(window, api.theme.ground, 0.03)
    page.back()
    until(lambda: root.property("subOpen") is False, "opened on a slot, B leaves the page rather than showing the cards")
    QMetaObject.invokeMethod(root, "openSub", Q_ARG("QVariant", "pages/ArtworkPage.qml"), Q_ARG("QVariant", {"game": game}))
    page = until(lambda: (p := root.findChild(QObject, "artworkPage")) is not None and p.property("level") == "slots" and p)
    page.setProperty("index", 0)
    page.moveAcross(1)
    page.moveDown()
    page.moveAcross(-1)
    page.moveAcross(1)
    assert page.property("index") == 3, "the box front remembers the row it was left from"
    page.openMenu()
    until(lambda: page.property("modal") is True)
    settle(window)
    window.close()


def test_the_settings_artwork_button_asks_then_fetches_and_stops(api, fake, monkeypatch):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "STEP_S", 0.15)

    fake.core._game("control")["media"].pop("logo")
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.setProperty("tabIndex", root.property("settingsTab"))
    page = page_as(root, "SettingsPage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "artwork"))
    store = api.screens.artworkOverview
    until(lambda: store.missingGames == 1)
    overview = until(lambda: page.findChild(QObject, "artworkOverview"))
    settle(window)
    QTest.keyClick(window, Qt.Key.Key_Right)
    QTest.keyClick(window, Qt.Key.Key_Up)
    until(lambda: overview.property("onButton") is True)
    assert overview.property("buttonLabel") == "Fetch missing art"
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: next(h["label"] for h in page.property("hints").toVariant()) == "Select", "the confirm has the focus")
    assert store.job is None, "nothing runs before the confirm"
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: store.job is not None and store.job["total"] > 0)
    until(lambda: overview.property("buttonLabel").startswith("Stop · "))
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: store.job["cancelled"] and overview.property("buttonDim") is True)
    until(lambda: store.job["ok"] is not None and overview.property("buttonLabel") == "Fetch missing art")
    assert store.job["message"].startswith("Stopped after ")
    settle(window)
    window.close()


def test_the_switch2_artwork_page_opens_a_slot_with_what_shows_first(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject

    api.theme.set("switch2")
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/ArtworkPage.qml"), Q_ARG("QVariant", {"gameId": "dead-cells"}))
    top = page_as(root, "ArtworkPage", "topPage")
    form = api.screens.artwork
    depth = root.property("depth")
    until(lambda: form.gameId == "dead-cells" and [s["slot"] for s in top.property("slots")][:2] == ["box_front", "square"])
    top.setProperty("index", 1)
    top.open()
    until(lambda: root.property("depth") == depth + 1)
    top = root.property("topPage")
    until(lambda: top.property("slot") == "square" and form.candidatesSlot == "square" and form.candidates)
    cells = until(lambda: (cells := top.property("cells").toVariant()) and len(cells) == 2 + len(form.candidates) and cells)
    assert cells[0]["kind"] == "now" and cells[1]["kind"] == "candidate" and cells[-1]["kind"] == "key"
    top.setProperty("cellIndex", 2)
    top.activate()
    until(
        lambda: top.property("cells").toVariant()[1]["kind"] == "under" and form.slot("square")["kind"] == "picked",
        "a pick puts the default under it",
    )
    assert top.property("cellIndex") == 3, "the ring stays on the candidate that was picked"
    lit(window, api.theme.ground, 0.02)
    settle(window)
    window.close()


def test_b_held_opens_the_power_menu_in_both_looks(api):
    from PySide6.QtCore import QObject, Qt
    from PySide6.QtTest import QTest

    def hold_until(shown, what):
        QTest.keyPress(window, Qt.Key.Key_Escape)
        until(shown, what)
        QTest.keyRelease(window, Qt.Key.Key_Escape)

    def hold(ms):
        QTest.keyPress(window, Qt.Key.Key_Escape)
        pump(ms)
        QTest.keyRelease(window, Qt.Key.Key_Escape)

    until(lambda: api.system.actions)
    _engine, window = render(api, activate=True)
    confirm = window.findChild(QObject, "confirm")
    hold(100)
    assert confirm.property("open") is False, "a tap is a tap"
    hold_until(lambda: confirm.property("open") is True, "held past the hold: the power menu")
    assert confirm.property("message") == "Power"
    assert [i["label"] for i in confirm.property("items").toVariant()] == ["Quit Universe", "Suspend", "Reboot", "Power off", "Stay"]
    assert confirm.property("index") == 0, "Quit Universe first, under the cursor"
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: confirm.property("open") is False, "B on the menu stays")
    api.theme.set("switch2")
    settle(window)
    picker = window.findChild(QObject, "picker")
    hold_until(lambda: picker.property("open") is True, "held past the hold: the power options")
    assert picker.property("title") == "Power Options"
    assert picker.property("choices").toVariant() == ["Quit Universe", "Sleep Mode", "Restart", "Turn Off"]
    hold(500)
    assert picker.property("open") is False, "held on the menu: B closes it and the hold asks nothing more"
    dialog = window.findChild(QObject, "dialog")
    hold_until(lambda: picker.property("open") is True, "held again: the power options")
    for _ in range(3):
        QTest.keyClick(window, Qt.Key.Key_Down)
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: dialog.property("open") is True and dialog.property("message") == "Turn off the system?")
    assert dialog.property("index") == 0, "Cancel under the cursor"
    QTest.keyClick(window, Qt.Key.Key_Right)
    QTest.keyClick(window, Qt.Key.Key_Return)
    assert until(lambda: api.universe.core.powered) == ["power_off"]
    settle(window)
    window.close()


def test_reboot_and_power_off_ask_again_and_close_the_game_first(api, fake):
    from PySide6.QtCore import QObject, Qt
    from PySide6.QtTest import QTest

    def pick(downs):
        root.askPower()
        until(lambda: confirm.property("open") is True and confirm.property("message") == "Power")
        for _ in range(downs):
            QTest.keyClick(window, Qt.Key.Key_Down)
        QTest.keyClick(window, Qt.Key.Key_Return)

    def powered(n):
        until(lambda: len(fake.core.powered) >= n)
        return fake.core.powered

    failed = record(api.system.failed)
    until(lambda: api.system.actions)
    fake.launch("mirrors-edge", "")
    until(lambda: fake.currentSession)
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    confirm = window.findChild(QObject, "confirm")
    pick(1)
    assert powered(1) == ["suspend"]
    until(lambda: confirm.property("open") is False)
    assert fake.core.current(), "suspend leaves the game running"
    pick(3)
    until(lambda: confirm.property("open") is True and confirm.property("message") == "Power off the computer?")
    assert confirm.property("note") == "Mirror's Edge is closed first."
    assert confirm.property("index") == 0, "Cancel under the cursor: one A too many powers nothing off"
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: confirm.property("open") is False)
    assert fake.core.powered == ["suspend"]
    pick(2)
    until(lambda: confirm.property("message") == "Reboot the computer?")
    QTest.keyClick(window, Qt.Key.Key_Down)
    QTest.keyClick(window, Qt.Key.Key_Return)
    assert powered(2) == ["suspend", "reboot"]
    assert fake.core.current() is None, "the game is stopped before the reboot"
    fake.core.power_error = 'Operation inhibited by "nosleep"'
    api.system.run("suspend")
    assert until(lambda: failed) == [("suspend", 'Operation inhibited by "nosleep"')]
    settle(window)
    window.close()


def test_a_reprise_list_that_fits_shows_every_row_without_scrolling(api):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
    from PySide6.QtTest import QTest

    def fits(menu, what):
        until(lambda: menu.property("open") is True and menu.property("slide") == 0, what)
        flick = next(o for o in menu.findChildren(QObject) if o.metaObject().className().startswith("QQuickFlickable"))
        settle(window)
        assert flick.property("contentHeight") <= flick.property("height") + 0.5, (
            f"{what}: the rows need {flick.property('contentHeight')}, the panel leaves {flick.property('height')}"
        )
        assert flick.property("contentY") == 0, what

    def closed(menu):
        QTest.keyClick(window, Qt.Key.Key_Escape)
        until(lambda: menu.property("open") is False)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    confirm = window.findChild(QObject, "confirm")
    page_as(root, "HomePage")
    api.screens.controller.walkOffered.emit("x", "8BitDo Ultimate 2C")
    fits(confirm, "a question with a note")
    closed(confirm)
    root.askPower()
    fits(confirm, "a question alone")
    closed(confirm)
    spec = {"message": "A question that goes on " * 20, "detail": "A note that goes on and on. " * 200}
    QMetaObject.invokeMethod(confirm, "ask", Q_ARG("QVariant", spec), Q_ARG("QVariant", None))
    fits(confirm, "a question with an endless note")
    closed(confirm)
    menu = window.findChild(QObject, "gameMenu")
    QTest.keyClick(window, Qt.Key.Key_F1)
    fits(menu, "a list beside its row")
    for key in (Qt.Key.Key_Down,) * 3 + (Qt.Key.Key_Return,):
        QTest.keyClick(window, key)
    until(lambda: menu.property("title") != "")
    fits(menu, "a titled list beside its row")
    settle(window)
    window.close()


def test_the_switch2_picker_opens_on_a_late_choice_with_its_ring_whole(api):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    _engine, window = render(api, activate=True)
    api.theme.set("switch2")
    settle(window)
    picker = window.findChild(QObject, "picker")
    title = "A title far longer than the card that holds it, and then some more words to be sure"
    spec = {"title": title, "choices": [f"Choice {i}" for i in range(12)], "index": 11}
    QMetaObject.invokeMethod(picker, "show", Q_ARG("QVariant", spec), Q_ARG("QVariant", None))
    flick = next(o for o in picker.findChildren(QObject) if o.metaObject().className().startswith("QQuickListView"))
    until(
        lambda: (
            flick.property("count") == 12
            and flick.property("contentY") + flick.property("height") == flick.property("originY") + flick.property("contentHeight")
        ),
        "the last row shows with the room under it",
    )
    settle(window)
    heading = next(o for o in picker.findChildren(QObject) if o.property("text") == title)
    assert heading.property("x") + heading.property("width") <= heading.parentItem().property("width"), "the title stays on the card"
    settle(window)
    window.close()


def test_a_switch2_dialog_taller_than_the_screen_scrolls_its_text(api):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
    from PySide6.QtTest import QTest

    def ask(spec):
        QMetaObject.invokeMethod(dialog, "show", Q_ARG("QVariant", spec), Q_ARG("QVariant", None))
        until(lambda: dialog.property("open") is True)
        settle(window)

    def closed():
        QTest.keyClick(window, Qt.Key.Key_Escape)
        until(lambda: dialog.property("open") is False)

    def end():
        return flick.property("contentHeight") - flick.property("height")

    # Each Down glides on from wherever the last one's glide got to: pressing on every look reads fastest.
    def read_on():
        if flick.property("contentY") == end():
            return True
        QTest.keyClick(window, Qt.Key.Key_Down)
        return False

    _engine, window = render(api, activate=True)
    api.theme.set("switch2")
    settle(window)
    dialog = window.findChild(QObject, "dialog")
    flick = next(o for o in dialog.findChildren(QObject) if o.metaObject().className().startswith("QQuickFlickable"))
    ask({"message": "Delete the save?", "buttons": ["Cancel", "Delete"]})
    QTest.keyClick(window, Qt.Key.Key_Down)
    assert flick.property("contentY") == 0, "a short question does not move"
    closed()
    ask({"message": "A question", "detail": "A detail that goes on and on. " * 100, "buttons": ["Cancel", "OK"]})
    until(lambda: flick.property("contentHeight") > flick.property("height"))
    card = flick.parentItem()
    assert card.property("height") <= window.height(), "the card stays on the screen"
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: flick.property("contentY") > 0, "Down reads on")
    until(read_on, "Down reaches the end")
    closed()
    ask({"message": "Again", "buttons": ["OK"]})
    until(lambda: flick.property("contentY") == 0, "a new question starts at its top")
    settle(window)
    window.close()


def test_the_switch2_folder_sheet_follows_the_chip_past_the_screen_edge(api, tmp_path, monkeypatch):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
    from PySide6.QtTest import QTest

    from universe_ui.screens import paths

    drives = [tmp_path / f"Drive {i:02}" for i in range(30)]
    for d in drives:
        d.mkdir()
    monkeypatch.setattr(paths, "_mounts", lambda: [str(d) for d in drives])
    _engine, window = render(api, activate=True)
    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "browse", Q_ARG("QVariant", {"path": str(drives[0])}), Q_ARG("QVariant", None))

    def texts():
        return [o for o in root.findChildren(QObject) if o.inherits("QQuickText") and o.property("visible")]

    views = (o for o in root.findChildren(QObject) if o.metaObject().className().startswith("QQuickListView"))
    chips = until(lambda: next((v for v in views if (v.property("count") or 0) >= 30), None))
    path = until(lambda: next((t for t in texts() if str(t.property("text")).endswith("Drive 00")), None))
    title = next(t for t in texts() if t.property("text") == "Choose a folder")
    settle(window)
    left = path.mapToItem(window.contentItem(), 0, 0).x()
    assert left >= title.mapToItem(window.contentItem(), title.property("width"), 0).x(), "the path stays clear of the title"
    QTest.keyClick(window, Qt.Key.Key_Up)
    for _ in range(chips.property("count")):
        QTest.keyClick(window, Qt.Key.Key_Right)
    assert chips.property("currentIndex") == chips.property("count") - 1

    def last_in_view():
        chip = chips.property("currentItem")
        return chip is not None and chip.property("x") + chip.property("width") <= chips.property("contentX") + chips.property("width")

    until(last_in_view, "the last chip is in view")
    settle(window)
    window.close()


def test_a_long_journal_paragraph_stops_above_the_hint_bar(api):
    _engine, window = render(api, 1280, 800, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = until(lambda: page_as(root, "HomePage").property("currentGame"))
    root.openSub("pages/RecordingsPage.qml", {"game": game, "session": ""})

    page = until(lambda: root.findChild(QQuickItem, "recordingsPage"))
    paragraph = page.findChild(QQuickItem, "journalParagraph")
    until(lambda: paragraph.property("text"), "the fixture recording has an entry")
    paragraph.setProperty("text", "A paragraph that keeps going. " * 60)
    until(lambda: paragraph.property("implicitHeight") > paragraph.property("height"), "the long paragraph is cut")
    bottom = paragraph.mapToItem(window.contentItem(), 0, paragraph.property("height")).y()
    hint = page.findChild(QQuickItem, "hintBar")
    assert bottom <= hint.mapToItem(window.contentItem(), 0, 0).y()
    root.closeSub()
    settle(window)
    window.close()


def test_the_power_menu_lists_what_logind_would_do(fake):
    from universe_ui.api import System

    fake.core.power_list = ["reboot"]
    system = System(fake)
    assert until(lambda: system.actions) == ["reboot"]


def test_reprise_about_shows_the_build(api, fake):
    _engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("settingsTab"))
    page = page_as(root, "SettingsPage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "quit"))
    until(lambda: page.property("sectionId") == "about", "Quit lives in About now")
    content = page.property("content").toVariant()
    keys = [r["key"] for r in content["rows"]]
    assert keys == ["version", "changelog", "setup", "power"]
    assert content["rows"][0]["display"] == fake.version()
    assert page.property("acceptLabel") == "", "nothing to select on the version"
    assert [s["id"] for s in page.property("sections").toVariant()][-4:] == ["sound", "storage", "doctor", "about"], "no Search, Updates or Quit section"
    power = keys.index("power")
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", power), Q_ARG("QVariant", content["rows"][power]))
    confirm = window.findChild(QObject, "confirm")
    until(lambda: confirm.property("open") is True and confirm.property("message") == "Power", "the row opens the menu B held opens")
    settle(window)
    window.close()


def test_the_reprise_game_menu_groups_its_rows_and_hides_the_media_a_game_has_none_of(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    def actions():
        return [i["action"] for i in menu.property("items").toVariant()]

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    menu = window.findChild(QObject, "gameMenu")
    page = page_as(root, "HomePage")
    until(lambda: game_id(page) == "the-technomancer")
    click(Qt.Key.Key_F1)
    until(lambda: menu.property("open") is True)
    items = menu.property("items").toVariant()
    assert [i["action"] for i in items] == ["play", "details", "favourite", "media", "manage"]
    assert [i.get("gap", False) for i in items] == [False, True, False, False, False], "play, then the rest"
    assert items[3]["more"] is True and items[4]["more"] is True
    click(Qt.Key.Key_Down, 3)
    click(Qt.Key.Key_Return)
    until(lambda: menu.property("open") is True and menu.property("title") == "Media" and len(menu.property("stack").toVariant()) == 1)
    counts = {kind: len(getattr(fake, kind)("the-technomancer")) for kind in ("screenshots", "recordings", "journal")}
    assert all(counts.values()), "the fixture game has every kind"
    assert actions() == list(counts), "the kinds the game has, no counts"
    click(Qt.Key.Key_Escape)
    until(lambda: menu.property("title") == "", "B comes back to the row that opened it")
    assert menu.property("open") is True and menu.property("index") == 3 and actions()[3] == "media", "B comes back to the row that opened it"
    click(Qt.Key.Key_Escape)
    until(lambda: menu.property("open") is False)

    game = api.allGames.byId("mini-metro")
    QMetaObject.invokeMethod(root, "openMenu", Q_ARG("QVariant", game), Q_ARG("QVariant", page.property("menuAnchor")))
    until(lambda: actions() == ["play", "details", "favourite", "manage"], "nothing to browse: no Media row")
    click(Qt.Key.Key_Down, 3)
    click(Qt.Key.Key_Return)
    until(lambda: menu.property("title") == "Manage" and actions() == ["settings", "artwork", "sessions", "data", "remove"])
    assert menu.property("items").toVariant()[4]["danger"] is True
    click(Qt.Key.Key_Down, 4)
    click(Qt.Key.Key_Return)
    until(lambda: menu.property("open") is True and menu.property("title") == "Remove Mini Metro?")
    click(Qt.Key.Key_Down)
    click(Qt.Key.Key_Return)
    until(lambda: menu.property("open") is False)
    until(lambda: api.allGames.byId("mini-metro") is None)
    settle(window)
    window.close()


def game_settings(window, root, args):
    """The game's settings sub-page, landed on `args["key"]` when there is one."""
    root.openSub("pages/GameSettingsPage.qml", args)
    page = until(lambda: window.findChild(QObject, "gameSettingsPage"))
    until(lambda: (row := page.property("row")) and row["key"] == args.get("key", row["key"]), f"landed on {args.get('key')}")
    settle(window)
    return page


def test_the_game_settings_page_applies_a_value_to_all_games(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def click(key):
        QTest.keyClick(window, key)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    page = game_settings(window, root, {"game": api.allGames.byId("the-technomancer"), "key": "launch.ntsync"})
    messages = []
    page.message.connect(messages.append)
    click(Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"]["ntsync"] is False and page.property("canPromote") is True)
    click(Qt.Key.Key_F1)
    click(Qt.Key.Key_Down)
    click(Qt.Key.Key_Return)
    until(lambda: fake.config()["set"]["launch"]["ntsync"] is False and "ntsync" not in fake.game("the-technomancer")["launch"])
    until(lambda: page.property("row")["origin"] == "global" and page.property("canPromote") is False)
    assert len(messages) == 1, "the form's word on what came of it, once"
    settle(window)
    window.close()


def test_the_game_settings_page_lands_a_search_hit_behind_advanced(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    page = game_settings(window, root, {"game": game, "key": "launch.ntsync"})
    body = page.findChild(QObject, "cardSections")
    form = api.screens.gameSettings
    assert form.showAdvanced is True, "an advanced row: Advanced comes on"
    sections = [s["name"] for s in page.property("sections").toVariant()]
    assert sections == ["Display", "Overlay", "Proton", "Launch", "Desktop and library", "Video capture", "Play journal", "Screenshots", "GOG"], (
        "no advanced card of its own"
    )
    assert sections[body.property("section")] == "Proton", "the hit sits in the Proton card, the cursor on it"
    assert [h["label"] for h in page.property("hints").toVariant()] == ["Toggle", "More", "Back"]
    assert page.property("canReset") is False, "nothing of the game's to drop"
    assert [i["action"] for i in page.property("moreItems").toVariant()] == ["advanced"], "More lists what X and Y do here"
    click(Qt.Key.Key_Return)
    until(
        lambda: fake.game("the-technomancer")["launch"]["ntsync"] is False and page.property("row")["origin"] == "game",
        "changing the value is what sets it on the game",
    )
    assert page.property("canReset") is True
    click(Qt.Key.Key_F1)
    menu = until(lambda: next((c for c in page.findChildren(QObject) if c.property("stack") is not None and c.property("open")), None))
    assert [i["action"] for i in menu.property("items").toVariant()] == ["reset", "promote", "advanced"]
    click(Qt.Key.Key_Escape)
    until(lambda: menu.property("open") is False)
    click(Qt.Key.Key_I)
    until(lambda: "ntsync" not in fake.game("the-technomancer")["launch"] and page.property("row")["origin"] == "default", "X drops it")
    click(Qt.Key.Key_F)
    until(lambda: form.showAdvanced is False and page.property("row")["key"] == "launch.proton", "the cursor lands on the card's first row")
    assert sections == [s["name"] for s in page.property("sections").toVariant()], "Y: the rows go, the sidebar stays"
    assert form.groups[body.property("section")]["title"] == "Proton"
    click(Qt.Key.Key_Escape)
    until(lambda: [h["label"] for h in page.property("hints").toVariant()] == ["Open", "Back"], "B: the sidebar")
    click(Qt.Key.Key_PageUp)
    until(lambda: form.groups[body.property("section")]["title"] == "Overlay", "LT steps the card")
    click(Qt.Key.Key_Escape)
    until(lambda: root.property("subOpen") is False, "B from the sidebar closes the page")
    root.openSub("pages/GameSettingsPage.qml", {"game": game})
    until(lambda: root.property("subOpen") is True and form.showAdvanced is False, "reopened, Advanced starts hidden")
    settle(window)
    window.close()


def test_the_game_settings_page_shows_an_advanced_change_with_advanced_off(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    page = game_settings(window, root, {"game": api.allGames.byId("the-technomancer"), "key": "launch.proton"})
    body = page.findChild(QObject, "cardSections")
    form = api.screens.gameSettings
    click(Qt.Key.Key_Down, 3)
    until(lambda: page.property("row")["key"] == "launch.prefix", "the game's own prefix sits under the Proton card's rows")
    assert form.showAdvanced is False
    click(Qt.Key.Key_I)
    until(
        lambda: "prefix" not in fake.game("the-technomancer")["launch"] and page.property("row")["key"] == "launch.proton",
        "reset, the row goes back behind Advanced and the cursor to the card's first row",
    )
    assert form.showAdvanced is False and form.groups[body.property("section")]["title"] == "Proton"
    settle(window)
    window.close()


def test_the_game_settings_page_adds_a_variable_from_one_sheet(api, fake):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    def type_text(text):
        for ch in text:
            QTest.keyClick(window, ch)

    def labels():
        return [h["label"] for h in page.property("hints").toVariant()]

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    page = game_settings(window, root, {"game": game, "key": "launch.env"})
    body = page.findChild(QObject, "cardSections")
    form = api.screens.gameSettings
    row = page.property("row")
    assert form.groups[body.property("section")]["title"] == "Launch" and row["map"] is True and row["label"] == "Add a variable…", (
        "the environment folds into Launch; the hit lands on the row that adds a variable"
    )
    assert labels()[:2] == ["Add", "More"]
    click(Qt.Key.Key_Return)
    until(lambda: labels() == ["Next", "Cancel"], "one sheet, two fields: the name first")
    type_text("DXVK_HUD")
    click(Qt.Key.Key_Return)
    until(lambda: labels() == ["Save", "Cancel"], "then the value")
    type_text("fps")
    click(Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"].get("env") == {"DXVK_HUD": "fps"})
    until(lambda: page.property("row")["key"] == "launch.env.DXVK_HUD", "the new variable is a row of its own, the cursor on it")
    assert page.property("row")["origin"] == "game"
    assert labels()[:2] == ["Change", "More"]
    assert page.property("moreItems").toVariant()[0]["label"] == "Remove", "More lists the variable's removal"
    click(Qt.Key.Key_I)
    until(lambda: fake.game("the-technomancer")["launch"].get("env", {}) == {}, "X removes it")
    settle(window)
    window.close()


def test_a_path_row_is_typed_first_under_a_keyboard(api, fake):
    from PySide6.QtCore import QCoreApplication, Qt
    from PySide6.QtTest import QTest

    from universe_ui import gamepad

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    def labels():
        return [h["label"] for h in page.property("hints").toVariant()]

    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    page = game_settings(window, root, {"game": game, "key": "launch.exe"})
    assert page.property("row")["type"] == "path"
    gamepad.post_key(Qt.Key.Key_Return, True, window=window)
    gamepad.post_key(Qt.Key.Key_Return, False, window=window)
    QCoreApplication.sendPostedEvents()
    until(lambda: labels() == ["Up", "More", "Cancel"], "under a pad the folders come first")
    assert api.keys.mode == "pad"
    click(Qt.Key.Key_Escape)
    click(Qt.Key.Key_Return)
    until(lambda: labels() == ["Done", "Browse", "Cancel"], "under a keyboard the path is typed first")
    assert api.keys.mode == "keyboard"
    click("2")
    click(Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"]["exe"] == "/mnt/games/PC/The Technomancer/TheTechnomancer.exe2", "the field held the value")
    click(Qt.Key.Key_Return)
    until(lambda: labels() == ["Done", "Browse", "Cancel"])
    click(Qt.Key.Key_F1)
    until(lambda: labels() == ["Up", "More", "Cancel"] and api.screens.paths.files is True, "F1: the folders, for a file")
    click(Qt.Key.Key_F)
    until(lambda: labels() == ["Done", "Browse", "Cancel"], "Y: back to typing")
    click(Qt.Key.Key_Escape)
    until(lambda: labels()[0] == "Change")
    assert warnings == []
    settle(window)
    window.close()


def test_the_sheets_type_the_physical_keyboards_letters(fake, tmp_path, monkeypatch):
    from PySide6.QtCore import Q_ARG, QCoreApplication, QMetaObject, Qt
    from PySide6.QtTest import QTest

    from universe_ui import gamepad
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    def press(key=Qt.Key.Key_Return):
        gamepad.post_key(key, True, window=window)
        gamepad.post_key(key, False, window=window)
        QCoreApplication.sendPostedEvents()

    def hints():
        return [h["label"] for h in page.property("hints").toVariant()]

    monkeypatch.setenv("XKB_DEFAULT_LAYOUT", "fr")
    monkeypatch.setenv("XKB_DEFAULT_VARIANT", "")
    exe = tmp_path / "The Technomancer" / "TheTechnomancer.exe"
    exe.parent.mkdir()
    fake.set("the-technomancer", "launch.exe", str(exe))
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    game = api.allGames.byId("the-technomancer")
    page = game_settings(window, root, {"game": game, "key": "launch.exe"})
    press()
    until(lambda: hints() == ["Up", "More", "Cancel"])
    click(Qt.Key.Key_F)
    until(lambda: "Browse" in hints(), "Y on the folders: the keyboard sheet")
    press()
    click(Qt.Key.Key_Down, 4)
    press()
    click(Qt.Key.Key_Up, 4)
    press()
    press(Qt.Key.Key_F)
    until(
        lambda: fake.game("the-technomancer")["launch"]["exe"] == os.path.dirname(exe) + "a&",
        "the first letter key is A, and ⇧ on the number row types the layout's own level",
    )

    api.theme.set("switch2")
    settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsSearchPage.qml"), Q_ARG("QVariant", {}))
    page_as(root, "SettingsSearchPage", "topPage")
    settle(window)
    press()
    click(Qt.Key.Key_Down, 3)
    press()
    click(Qt.Key.Key_Up, 4)
    press()
    press()
    until(lambda: api.screens.search.query == "a&1", "the Switch's ⇧ types one key")
    settle(window)
    window.close()
    api.shutdown()


def test_the_switch2_forms_share_the_sidebar_and_y(api, fake):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    def click(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    def push(source, args):
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
        page = page_as(root, source.removeprefix("pages/").removesuffix(".qml"), "topPage")
        settle(window)
        return page

    def labels(page):
        return [h["label"] for h in page.property("hints").toVariant()]

    def sections(page):
        return [s["label"] for s in page.property("sections").toVariant()]

    api.theme.set("switch2")
    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    form = api.screens.runner
    page = push("pages/FormPage.qml", {"runner": "proton"})
    until(lambda: sections(page) == ["Runner", "Builds", "Proton", "Games"])
    assert form.showAdvanced is False
    click(Qt.Key.Key_F)
    until(lambda: form.showAdvanced is True)
    assert sections(page) == ["Runner", "Builds", "Proton", "Games"], "Y: the sidebar stays"
    click(Qt.Key.Key_Right)
    click(Qt.Key.Key_Down, 3)
    row = until(lambda: (row := page.property("currentRow").toVariant()) and row["key"] == "gamescope" and row)
    assert row["origin"] == "default" and labels(page) == ["Hide advanced", "Reset", "Back", "Toggle"], "config.toml leaves gamescope to its default"
    click(Qt.Key.Key_Return)
    until(lambda: form.rows[row["form"]]["origin"] == "runner", "toggling the inherited switch sets it on the runner")
    click(Qt.Key.Key_I)
    until(lambda: form.rows[row["form"]]["origin"] == "default", "X clears it back")
    click(Qt.Key.Key_Escape, 2)
    settle(window)
    page = push("pages/FormPage.qml", {"source": "gog"})
    until(lambda: sections(page) == ["Settings", "Game defaults", "Sign-in"] and labels(page) == ["Show advanced", "Back", "OK"])
    click(Qt.Key.Key_Escape)
    settle(window)
    push("pages/SettingsPage.qml", {"section": "launch"})
    click(Qt.Key.Key_F)
    until(lambda: api.screens.launch.showAdvanced is True, "Y opens the Launch section's advanced rows")
    assert warnings == []
    settle(window)
    window.close()


# The fixture's pad supply hangs off event30: the fake pad's node, so its glyph goes green while that pad is current.
def test_the_badge_tints_the_current_pad(api):
    from PySide6.QtGui import QColor

    from universe_ui.screens.controller import FakeWatcher

    _engine, window = render(api)
    controller = api.screens.controller
    controller.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    controller.start(watcher)
    badge = until(lambda: next((c for c in window.findChildren(QObject) if c.property("currentTint") is not None), None))

    def rows():
        return [c for c in badge.childItems() if c.property("low") is not None]

    until(lambda: {c.property("current") for c in rows()} == {True, False}, "the laptop and the pad")
    sources = {c.property("current"): c for c in rows()}
    assert sources[True].property("ink") == badge.property("currentTint") and sources[False].property("ink") == badge.property("tint")
    watcher.emit({"event": "gone", "id": "event30"})
    until(lambda: not any(c.property("current") for c in rows()), "no pad current")
    assert len(rows()) == 2, "the kernel still reads the pad's charge"
    pad = rows()[1]
    until(lambda: pad.property("ink") == QColor(badge.property("tint")), "no pad current, no green")
    settle(window)
    window.close()


def test_the_sound_section_plays_through_the_output_picked_in_both_looks(api, fake):
    from PySide6.QtTest import QTest

    def outputs():
        content = page.property("content")
        return content.toVariant()["rows"] if content is not None else []

    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    root.goToTab(root.property("settingsTab"))
    page = page_as(root, "SettingsPage")
    QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "sound"))
    until(
        lambda: (
            [(r.get("label"), r.get("display"), r.get("tag")) for r in outputs()]
            == [
                ("Speakers", "Built-in Audio", "In use"),
                ("Headphones", "Built-in Audio", ""),
                ("HDMI / DisplayPort", "TV", ""),
            ]
        )
    )
    assert page.property("acceptLabel") == "", "the output in use: nothing to do"
    QTest.keyClick(window, Qt.Key.Key_Down)
    until(lambda: page.property("acceptLabel") == "Use")
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: [r["label"] for r in outputs() if r["tag"]] == ["Headphones"])
    settle(window)
    window.close()

    api.theme.set("switch2")
    _engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "sound"}))
    page = page_as(root, "SettingsPage", "topPage")

    def rows():
        content = page.property("content")
        return content.toVariant() if content is not None else []

    until(
        lambda: (
            [(r.get("label"), r.get("path"), r.get("value")) for r in rows()]
            == [
                ("Speakers", "Built-in Audio", False),
                ("Headphones", "Built-in Audio", True),
                ("HDMI / DisplayPort", "TV", False),
            ]
        )
    )
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", 2), Q_ARG("QVariant", rows()[2]))
    until(lambda: [r["label"] for r in rows() if r["value"]] == ["HDMI / DisplayPort"])
    settle(window)
    window.close()


@pytest.mark.parametrize("look", ["reprise", "switch2", "ps5"])
def test_in_the_universe_session_the_way_out_logs_out(universe_session, api, look):
    until(lambda: api.system.actions)
    if look != "reprise":
        api.theme.set(look)
        api.theme.takeLanding()
    _engine, window = render(api)
    root = window.property("contentItem").childItems()[0].property("item")
    items = QMetaObject.invokeMethod(root, "powerItems", Qt.DirectConnection, Q_RETURN_ARG("QVariant"))
    keys = [i.get("action", i.get("act")) for i in (items.toVariant() if hasattr(items, "toVariant") else items)]
    way_out = "logout" if universe_session else "quit"
    assert way_out in keys and "power_off" in keys, keys
    assert ("quit" if universe_session else "logout") not in keys, "one way out of the launcher"
    settle(window)
    window.close()


@pytest.mark.parametrize("look", ["reprise", "switch2", "ps5"])
def test_each_looks_themes_page_says_it_is_not_affiliated(api, look):
    _engine, window = render(api)
    for step in ["ps5", "reprise"] if look == "reprise" else [look]:
        api.theme.set(step)
        settle(window)
    root = window.property("contentItem").childItems()[0].property("item")
    slot = "activePage" if look == "reprise" else "topPage"
    page = until(lambda: (p := root.property(slot)) is not None and p.property("sectionId") == "themes" and p, "a switch lands on the look's Themes")
    content = js(page, "content")
    rows = content["rows"] if isinstance(content, dict) else content
    assert any(r.get("key") == "affiliation" for r in rows)
    settle(window)
    window.close()


@pytest.mark.parametrize(("look", "menu"), [("reprise", "menu"), ("switch2", "picker"), ("ps5", "popup")])
def test_a_runner_pages_builds_card_installs_after_asking(api, fake, look, menu):
    from PySide6.QtTest import QTest

    components = api.screens.components
    components.load()
    until(lambda: not components.busy)
    if look != "reprise":
        api.theme.set(look)
        api.theme.takeLanding()
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    if look == "reprise":
        root.openSub("pages/FormPage.qml", {"runner": "rpcs3"})
        page = until(lambda: next((i for i in root.findChildren(QQuickItem) if i.metaObject().className().startswith("FormPage")), None))
    else:
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/FormPage.qml"), Q_ARG("QVariant", {"runner": "rpcs3"}))
        page = page_as(root, "FormPage", "topPage")
    form = api.screens.runner
    index = until(lambda: next((i for i, r in enumerate(form.rows) if r.get("component") == "rpcs3"), None), "the Builds card")
    assert next(g for g in form.groups if index in g["rows"])["title"] == "Builds"
    settle(window)
    finished = record(fake.jobFinished)
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", index), Q_ARG("QVariant", form.rows[index]))
    owner = page if look == "reprise" else root
    until(lambda: owner.findChild(QObject, menu).property("open") is True, "A opens the build's options")
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: owner.findChild(QObject, "dialog").property("open") is True, "the install asks first: its size, the room left")
    assert components.job is None
    QTest.keyClick(window, Qt.Key.Key_Return)
    until(lambda: components.job is not None and components.job["component"] == "rpcs3", "Install is the default")
    until(lambda: finished)
    until(lambda: not components.busy)
    until(lambda: not next(r for r in form.rows if r.get("component") == "rpcs3")["accent"], "in: nothing left to do")
    settle(window)
    window.close()
