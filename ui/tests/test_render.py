import os

import pytest
from looks import LOOKS, MENU, Look, call, current_row, invoke, lit_fraction, read, render, settle
from PySide6.QtCore import QObject, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest

from conftest import own, pump, record, until
from universe_ui import host


def count_frames(window, ms):
    frames = []
    window.frameSwapped.connect(lambda: frames.append(1))
    pump(ms)
    window.frameSwapped.disconnect()
    return len(frames)


def game_id(page):
    game = page.property("currentGame")
    return game.property("id") if game is not None else None


@pytest.mark.slow
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


@pytest.mark.parametrize("name", LOOKS)
def test_a_switch_lands_once_on_the_new_look_s_themes_section(api, name):
    shown = Look(api, next(other for other in LOOKS if other != name))
    shown.switch(name)
    page = shown.page("settingsPage")
    until(lambda: page.property("sectionId") == "themes", "a switch lands on the new look's Themes")
    assert page.property("level") in (None, "section"), "on the section itself, not the list of sections"
    content = read(page, "content")
    rows = content["rows"] if isinstance(content, dict) else content
    assert {"sound_dir", "affiliation"} <= {r.get("key") for r in rows}
    assert api.theme.landing == "", "taken once"
    shown.close()


reprise = pytest.mark.parametrize("look", ["reprise"], indirect=True)
switch2 = pytest.mark.parametrize("look", ["switch2"], indirect=True)
stacked = pytest.mark.parametrize("look", ["switch2", "ps5"], indirect=True)


@pytest.mark.parametrize("skip", [pytest.param(True, id="skipped"), pytest.param(False, id="played", marks=pytest.mark.slow)])
@pytest.mark.parametrize("name", LOOKS)
def test_the_intro_plays_then_leaves_home_the_focus_and_no_press(fake, tmp_path, name, skip):
    from PySide6.QtTest import QTest

    from universe_ui.api import Api
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE

    def has_focus(item):
        at = look.window.activeFocusItem()
        while at is not None and at != item:
            at = at.parentItem()
        return at is not None

    api = Api(fake, memory_path=str(tmp_path / "memory.json"), theme=name, power_root=FAKE, net_root=FAKE_NET, boot=True)
    try:
        look = Look(api, name)
        boot = look.find("boot")
        home = look.home()
        start = until(lambda: game_id(home))
        assert api.boot.running and boot.property("visible")
        until(lambda: boot.property("ring") > 0, "the mark comes in")
        if skip:
            QTest.keyClick(look.window, Qt.Key.Key_Right)
            until(lambda: not api.boot.running, "a press skips it", 1000)
        else:
            until(lambda: not api.boot.running, "it ends by itself", 3000)
        assert game_id(home) == start, "the press that skipped stays the intro's"
        until(lambda: has_focus(home), "home holds the focus")
        QTest.keyClick(look.window, Qt.Key.Key_Right)
        until(lambda: game_id(home) != start, "the next press moves home")
        assert not boot.property("visible")
        look.switch("ps5" if name != "ps5" else "reprise")
        assert not api.boot.running and not boot.property("visible"), "a switch does not replay it"
        look.close()
    finally:
        api.shutdown()


@reprise
def test_the_media_tab_and_the_screenshots_page(look, api):
    root = look.root
    root.goToTab(1)
    page = look.page("mediaPage")
    until(lambda: not api.screens.media.loading and read(page, "rows"))
    rows = read(page, "rows")
    assert root.property("tabIndex") == 1 and read(page, "current")["kind"] in ("shot", "recording", "journal")
    assert {r["kind"] for r in rows} == {"shot", "recording", "journal"}, "one grid, every kind, no filter"
    assert [h["glyph"] for h in read(page, "hints")] == ["A", "Start", "B"]
    page.setProperty("index", next(i for i, r in enumerate(rows) if r["kind"] == "shot" and r["gameId"] == "the-technomancer"))
    page.open()
    until(lambda: page.property("lightbox") is True and page.property("modal") is True)
    page.setProperty("lightbox", False)
    game = page.property("currentGame")
    assert game is not None and game.property("id") == "the-technomancer"
    look.open("pages/ScreenshotsPage.qml", {"game": game, "name": read(page, "current")["name"]})
    assert root.property("subOpen") is True
    shots = api.screens.shots
    until(lambda: shots.gameId == "the-technomancer" and shots.count > 0, "the sub-page loaded the game's shots")
    root.closeSub()
    until(lambda: root.property("subOpen") is False)


@reprise
def test_the_tab_bar_search_finds_the_settings_under_the_games(look, api):
    root = look.root
    root.openSearch()
    overlay = look.find("searchOverlay")
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
    assert [h["glyph"] for h in read(overlay, "hints")] == ["A", "B"]
    look.press(Qt.Key.Key_Return)
    page = look.page("settingsPage")
    assert root.property("searchOpen") is False and root.property("tabIndex") == root.property("settingsTab")
    cards = page.findChild(QObject, "settingsCards")
    until(
        lambda: page.property("sectionId") == "launch" and (read(cards, "currentRow") or {}).get("key") == "launch.gamescope_adaptive_sync",
        "the hit's row, on its section",
    )


@reprise
def test_the_screenshots_page_puts_the_running_sessions_shots_first(look, api, fake):
    page = look.open("pages/ScreenshotsPage.qml", {"game": api.allGames.byId("the-technomancer")})
    earlier = until(lambda: read(page, "rows"))
    assert page.property("since") == "" and page.property("mine") == 0, "no session: one run, no headings"
    grid = next(c for c in page.findChildren(QObject) if c.property("cellHeight") is not None)
    card = until(lambda: call(grid, "currentCard"))
    picture = card.property("height") - card.property("captionHeight")
    assert abs(picture - card.property("width") * 9 / 16) < 1, "the cell makes room for the date line: the picture stays 16:9, whole"
    fake.launch("the-technomancer", "")
    until(lambda: api.universe.currentSession)
    api.home.screenshot()
    until(lambda: page.property("mine") == 1, "the playing game's page splits at the session's start")
    assert page.property("since") != ""
    rows = read(page, "rows")
    assert len(rows) == len(earlier) + 1 and rows[0]["name"] not in {r["name"] for r in earlier}, "the new shot leads"
    api.universe.stop(api.universe.currentSession["session_id"])
    until(lambda: page.property("since") == "", "the session over, one run again")


@switch2
def test_the_switch2_album_holds_the_shots_too(look):
    top = look.open("pages/AlbumPage.qml")
    shown = until(lambda: read(top, "shown"))
    assert {r["kind"] for r in shown} == {"shot", "recording"} and [r["when"] for r in shown] == sorted((r["when"] for r in shown), reverse=True)
    top.setProperty("kindFilter", "shot")
    until(lambda: (shown := read(top, "shown")) and all(r["kind"] == "shot" for r in shown))
    top.play()
    until(lambda: top.property("viewing") is True)


def test_a_session_running_at_startup_is_home_with_the_game_pinned(api, fake):
    fake.launch("mirrors-edge", "")
    until(lambda: fake.currentSession)
    look = Look(api, "reprise")
    overlay = look.find("launchOverlay")
    assert overlay.property("running") is False
    root = look.root
    assert root.property("playingId") == "mirrors-edge"
    until(lambda: game_id(look.home()) == "mirrors-edge")
    fake.core.end_session()
    until(lambda: root.property("playingId") == "")
    assert fake.currentSession is None
    look.close()


def test_the_cursor_lands_on_the_game_just_played_once_its_session_ends(look, fake):
    home = look.home()
    until(lambda: game_id(home) == "the-technomancer")
    look.launch("dead-cells")
    until(lambda: fake.currentSession and look.root.property("launching") is False)
    assert game_id(home) == "the-technomancer", "the cursor stays put under the game"
    fake.stop("")
    until(lambda: fake.currentSession is None and game_id(home) == "dead-cells", "back from the game, on the game")


@reprise
def test_a_launch_holds_the_poster_until_the_window_is_shown(look, fake, monkeypatch):
    import threading

    root, window = look.root, look.window
    overlay = look.find("launchOverlay")
    mapped, shown = threading.Event(), record(fake.sessionShown)
    wait_session_window = fake.core.wait_session_window
    monkeypatch.setattr(fake.core, "wait_session_window", lambda *args: (mapped.wait(5), wait_session_window(*args))[1])
    look.launch("control")
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


def test_the_overlay_draws_the_volume_level_inside_gamescope(api, fake, monkeypatch):
    from PySide6.QtCore import QRect, QSize
    from PySide6.QtQml import QQmlApplicationEngine

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
    assert api.home.osd


def test_signals_end_the_loop_while_it_idles(app):
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


def install_page(look):
    """The look's install page: Reprise's is a Settings section, the others' a page of its own."""
    if look.stacked:
        return look.open("pages/InstallPage.qml")
    return look.settings("install")


def install_titles(look, page):
    if look.name == "reprise":
        content = read(page, "content")
        return [r["label"] for r in content["rows"] if r.get("key") == "game"] if content else []
    if look.name == "switch2":
        return [c["game"]["title"] for c in read(page, "cells") or [] if not c.get("heading")]
    rows = look.api.screens.sources.rows
    return sorted({rows[int(i)]["title"] for c in read(page, "collections") or [] for i in c["items"]})


def test_a_second_store_switches_the_install_page_to_its_games(look, api, fake):
    fake.core._source("epic").update(enabled=True, logged_in=True)
    fake.core._data["source_library"]["epic"] = [{"id": "Min", "title": "Hades", "owned": True, "installed": False}]
    sources = api.screens.sources
    page = install_page(look)
    until(lambda: sources.rows and install_titles(look, page), "the first store's games")
    assert "Hades" not in install_titles(look, page)
    sources.pick("epic")
    until(lambda: [r["id"] for r in sources.rows] == ["Min"] and install_titles(look, page) == ["Hades"], "the second store's own")


def test_the_component_bar_hides_and_the_job_goes_on(look, api, fake):
    from PySide6.QtCore import QPointF

    from universe_ui import gamepad

    hide = {"reprise": Qt.Key.Key_F, "switch2": Qt.Key.Key_I, "ps5": Qt.Key.Key_I}[look.name]
    form = api.screens.components
    finished = record(fake.jobFinished)
    page = look.settings("components")
    until(lambda: page.property("sectionId") == "runners", "a landing on Components opens Runners")

    def install(ident):
        until(form.listing)
        assert form.act(ident, "install")
        until(lambda: page.property("componentsBar") is True)
        settle(look.window)

    def hidden():
        return form.job is None and page.property("componentsBar") is False

    install("wine")
    look.press(hide)
    until(hidden, "its button hides it")
    until(lambda: len(finished) == 1, "the job goes on")
    install("umu-run")
    closer = until(lambda: (c := page.findChild(QQuickItem, "hideJob")) and c.isVisible() and c)
    p = closer.mapToScene(QPointF(closer.width() / 2, closer.height() / 2))
    if look.stacked:
        gamepad.touch(look.window, [(p.x(), p.y())], 0)
    else:
        QTest.mouseClick(look.window, Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier, p.toPoint())
    until(hidden, "so does its ×")
    until(lambda: len(finished) == 2)
    if look.stacked:
        invoke(look.root, "componentAction", "xemu", "uninstall")
        dialog = look.dialog()
        until(lambda: dialog.property("open") is True, "the uninstall asks first")
        assert dialog.property("dangerIndex") == 1, "Uninstall is the red button"


def test_an_install_runs_one_at_a_time_and_x_cancels_it_keeping_its_files(look, api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "STEP_S", 0.15)
    sources = api.screens.sources
    finished = record(fake.jobFinished)
    page = install_page(look)
    until(lambda: sources.rows)
    stardew = next(i for i, r in enumerate(sources.rows) if r["title"] == "Stardew Valley")
    witcher = next(i for i, r in enumerate(sources.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    ident = sources.rows[stardew]["id"]
    sources.install(stardew)
    until(lambda: sources.job and sources.job["game"] == ident)
    said = record(sources.message)
    assert sources.install(witcher) == "" and len(said) == 1, "one job at a time: the second is refused, with a word why"
    assert sources.job["game"] == ident
    if look.stacked:
        until(lambda: "X" in [h["glyph"] for h in read(page, "hints")], "X cancels while a job runs")
        look.press(Qt.Key.Key_I)
    else:
        assert sources.cancel()
    until(lambda: finished and sources.job["cancelled"])
    until(lambda: next(r for r in sources.rows if r["id"] == ident)["partial"], "the files stay for a later resume")


@reprise
def test_the_reprise_library_leads_with_hearts_and_y_hearts_the_game_under_the_cursor(look, api):
    def title():
        game = page.property("currentGame")
        return game.property("title") if game is not None else None

    root = look.root
    root.goToTab(root.property("libraryTab"))
    page = look.page("libraryPage")
    until(lambda: title() == "Dead Cells", "the hearted games first, by title")
    look.press(Qt.Key.Key_Right, 2)
    until(lambda: title() == "Batman: Arkham Origins", "then the rest, by title")
    assert [h["glyph"] for h in read(page, "hints")] == ["A", "Start", "B"]
    look.press(Qt.Key.Key_F)
    until(lambda: api.allGames.byId("batman-arkham-origins").favorite is True)
    until(lambda: title() == "Batman: Arkham Origins", "the cursor follows the game to its place among the hearts")
    look.press(Qt.Key.Key_Right)
    until(lambda: title() == "Dead Cells")


def library(look):
    if look.stacked:
        return look.open({"switch2": "pages/AllSoftwarePage.qml", "ps5": "pages/LibraryPage.qml"}[look.name])
    look.root.goToTab(look.root.property("libraryTab"))
    return look.page("libraryPage")


def test_the_right_stick_pages_the_library_from_a_clamp_at_its_top(look):
    def cursor():
        if not look.stacked:
            return game_id(page)
        current = grid.property("current")
        return current.property("id") if isinstance(current, QObject) else (read(grid, "current") or {}).get("id")

    warnings = record(look.engine.warnings)
    page = library(look)
    grid = page.findChild(QObject, "grid")
    first = until(cursor)
    look.press(Qt.Key.Key_BracketLeft)
    assert cursor() == first, "the first row is a clamp"
    look.press(Qt.Key.Key_BracketRight)
    until(lambda: cursor() != first, "a screenful down")
    look.press(Qt.Key.Key_BracketLeft)
    until(lambda: cursor() == first, "and back up")
    assert warnings == []


@reprise
def test_down_past_the_last_reprise_library_row_lands_on_the_add_tile_and_back_on_its_column(look):
    page = library(look)
    first = until(lambda: game_id(page))
    # Eight games on eight columns: the add tile alone on the second row.
    look.press(Qt.Key.Key_Right, 3)
    look.press(Qt.Key.Key_BracketRight)
    until(lambda: page.property("onAddTile") is True, "down past the last game's row lands on the last cell")
    look.press(Qt.Key.Key_BracketRight, 3)
    assert page.property("onAddTile") is True
    look.press(Qt.Key.Key_BracketLeft)
    until(lambda: game_id(page) == first, "back up from the last cell: its own column, the first")


@reprise
def test_the_reprise_artwork_page_opens_on_a_slot_and_lists_its_candidates(look, api):
    root = look.root
    game = api.allGames.byId("the-technomancer")
    page = look.open("pages/ArtworkPage.qml", {"game": game, "slot": "logo"})
    form = api.screens.artwork
    until(lambda: form.gameId == "the-technomancer")
    assert page.property("level") == "browser", "opened on a slot, the page went straight to its candidates"
    until(lambda: form.candidatesSlot == "logo" and form.candidates)
    page.back()
    until(lambda: root.property("subOpen") is False, "opened on a slot, B leaves the page rather than showing the cards")
    page = look.open("pages/ArtworkPage.qml", {"game": game})
    until(lambda: page.property("level") == "slots")
    page.setProperty("index", 0)
    page.moveAcross(1)
    page.moveDown()
    page.moveAcross(-1)
    page.moveAcross(1)
    assert page.property("index") == 3, "the box front remembers the row it was left from"
    page.openMenu()
    until(lambda: page.property("modal") is True)


@stacked
def test_an_artwork_slot_lists_what_shows_then_the_candidates_and_a_pick_puts_the_default_under_it(look, api, fake):
    form = api.screens.artwork
    page = look.open("pages/ArtworkPage.qml", {"gameId": "dead-cells"})
    until(lambda: form.gameId == "dead-cells" and [s["slot"] for s in read(page, "slots")][:2] == ["box_front", "square"])
    page.setProperty("index", 1)
    look.press(Qt.Key.Key_Return)
    top = look.page("artworkSlotPage")
    until(lambda: top.property("slot") == "square" and form.candidatesSlot == "square" and form.candidates)
    cells = until(lambda: (cells := read(top, "cells")) and len(cells) == 2 + len(form.candidates) and cells)
    assert cells[0]["kind"] == "now" and cells[1]["kind"] == "candidate" and cells[-1]["kind"] == "key"
    top.setProperty("cellIndex", 2)
    picked = record(fake.mediaChanged)
    look.press(Qt.Key.Key_Return)
    until(lambda: picked)
    until(lambda: read(top, "cells")[1]["kind"] == "under" and form.slot("square")["kind"] == "picked", "a pick puts the default under it")
    until(lambda: top.property("cellIndex") == 3, "the ring stays on the candidate that was picked")


@reprise
def test_the_settings_artwork_button_asks_then_fetches_and_stops(look, api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "STEP_S", 0.15)
    fake.core._game("control")["media"].pop("logo")
    page = look.settings("artwork")
    store = api.screens.artworkOverview
    until(lambda: store.missingGames == 1)
    overview = until(lambda: page.findChild(QObject, "artworkOverview"))
    settle(look.window)
    look.press(Qt.Key.Key_Right)
    look.press(Qt.Key.Key_Up)
    until(lambda: overview.property("onButton") is True)
    look.press(Qt.Key.Key_Return)
    until(lambda: page.findChild(QObject, "dialog").property("open") is True, "the fetch asks first")
    assert store.job is None, "nothing runs before the confirm"
    look.press(Qt.Key.Key_Return)
    until(lambda: store.job is not None and store.job["total"] > 0)
    look.press(Qt.Key.Key_Return)
    until(lambda: store.job["cancelled"] and overview.property("buttonDim") is True, "A again stops it")
    until(lambda: store.job["ok"] is not None and overview.property("buttonDim") is False)


POWER_MENU = {"reprise": "confirm", "switch2": "picker", "ps5": "popup"}


def test_b_held_opens_the_power_menu_and_power_off_asks_again_cancel_first(look, api):
    def hold(ms=0, until_shown=None):
        QTest.keyPress(look.window, Qt.Key.Key_Escape)
        if until_shown:
            until(until_shown, "held past the hold: the power menu")
        else:
            pump(ms)
        QTest.keyRelease(look.window, Qt.Key.Key_Escape)

    until(lambda: api.system.actions)
    menu = look.find(POWER_MENU[look.name])
    dialog = look.dialog()
    keys = [i.get("action", i.get("act")) for i in call(look.root, "powerItems")]
    hold(100)
    assert menu.property("open") is False, "a tap is a tap"
    hold(until_shown=lambda: menu.property("open") is True)
    hold(500)
    assert menu.property("open") is False, "held on the menu: B closes it and the hold asks nothing more"
    hold(until_shown=lambda: menu.property("open") is True)
    look.press(Qt.Key.Key_Down, keys.index("power_off"))
    look.press(Qt.Key.Key_Return)
    until(
        lambda: dialog.property("open") is True and dialog.property("index") == 0 and not (dialog is not menu and menu.property("open")),
        "power off asks again, Cancel under the cursor",
    )
    look.press(Qt.Key.Key_Right if look.stacked else Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    assert until(lambda: api.universe.core.powered) == ["power_off"]


@reprise
def test_reboot_and_power_off_close_the_game_first_and_suspend_leaves_it(look, api, fake):
    confirm = look.find("confirm")

    def pick(action):
        look.root.askPower()
        until(lambda: confirm.property("open") is True and read(confirm, "items")[0]["action"] == "quit")
        look.press(Qt.Key.Key_Down, [i["action"] for i in read(confirm, "items")].index(action))
        look.press(Qt.Key.Key_Return)

    def powered(n):
        until(lambda: len(fake.core.powered) >= n)
        return fake.core.powered

    failed = record(api.system.failed)
    until(lambda: api.system.actions)
    fake.launch("mirrors-edge", "")
    until(lambda: fake.currentSession)
    pick("suspend")
    assert powered(1) == ["suspend"]
    until(lambda: confirm.property("open") is False)
    assert fake.core.current(), "suspend leaves the game running"
    pick("power_off")
    until(
        lambda: confirm.property("open") is True and confirm.property("index") == 0 and "Mirror's Edge" in confirm.property("note"),
        "the question names the game it closes",
    )
    look.press(Qt.Key.Key_Return)
    until(lambda: confirm.property("open") is False)
    assert fake.core.powered == ["suspend"], "Cancel under the cursor: one A too many powers nothing off"
    pick("reboot")
    until(lambda: confirm.property("open") is True and confirm.property("index") == 0)
    look.press(Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    assert powered(2) == ["suspend", "reboot"]
    assert fake.core.current() is None, "the game is stopped before the reboot"
    fake.core.power_error = 'Operation inhibited by "nosleep"'
    api.system.run("suspend")
    assert until(lambda: failed)[0][0] == "suspend"


@reprise
def test_a_reprise_list_that_fits_shows_every_row_without_scrolling(look, api):
    def fits(menu, what):
        until(lambda: menu.property("open") is True and menu.property("slide") == 0, what)
        flick = menu.findChild(QObject, "scroll")
        settle(look.window)
        assert flick.property("contentHeight") <= flick.property("height") + 0.5, (
            f"{what}: the rows need {flick.property('contentHeight')}, the panel leaves {flick.property('height')}"
        )
        assert flick.property("contentY") == 0, what

    def closed(menu):
        look.press(Qt.Key.Key_Escape)
        until(lambda: menu.property("open") is False)

    confirm = look.find("confirm")
    look.home()
    api.screens.controller.walkOffered.emit("x", "8BitDo Ultimate 2C")
    fits(confirm, "a question with a note")
    closed(confirm)
    look.root.askPower()
    fits(confirm, "a question alone")
    closed(confirm)
    invoke(confirm, "ask", {"message": "A question that goes on " * 20, "detail": "A note that goes on and on. " * 200}, None)
    fits(confirm, "a question with an endless note")
    closed(confirm)
    menu = look.find("gameMenu")
    look.press(Qt.Key.Key_F1)
    fits(menu, "a list beside its row")
    look.press(Qt.Key.Key_Down, 3)
    look.press(Qt.Key.Key_Return)
    until(lambda: len(read(menu, "stack")) == 1)
    fits(menu, "a titled list beside its row")


@switch2
def test_the_switch2_picker_opens_on_a_late_choice_with_its_ring_whole(look):
    picker = look.find("picker")
    title = "A title far longer than the card that holds it, and then some more words to be sure"
    invoke(picker, "show", {"title": title, "choices": [f"Choice {i}" for i in range(12)], "index": 11}, None)
    choices = picker.findChild(QObject, "choices")
    until(
        lambda: (
            choices.property("count") == 12
            and choices.property("contentY") + choices.property("height") == choices.property("originY") + choices.property("contentHeight")
        ),
        "the last row shows with the room under it",
    )
    settle(look.window)
    heading = next(o for o in picker.findChildren(QObject) if o.property("text") == title)
    assert heading.property("x") + heading.property("width") <= heading.parentItem().property("width"), "the title stays on the card"


@stacked
def test_a_dialog_taller_than_the_screen_scrolls_its_text(look):
    dialog = look.dialog()
    flick = dialog.findChild(QObject, "scroll")

    def ask(spec):
        invoke(dialog, "show", spec, None)
        until(lambda: dialog.property("open") is True)
        settle(look.window)

    def closed():
        look.press(Qt.Key.Key_Escape)
        until(lambda: dialog.property("open") is False)

    def read_on():
        if flick.property("contentY") >= flick.property("contentHeight") - flick.property("height"):
            return True
        look.press(Qt.Key.Key_Down)
        return False

    ask({"message": "Delete the save?", "buttons": ["Cancel", "Delete"]})
    look.press(Qt.Key.Key_Down)
    assert flick.property("contentY") == 0, "a short question does not move"
    closed()
    ask({"message": "A question", "detail": "A detail that goes on and on. " * 100, "buttons": ["Cancel", "OK"]})
    until(lambda: flick.property("contentHeight") > flick.property("height"))
    assert flick.parentItem().property("height") <= look.window.height(), "the card stays on the screen"
    look.press(Qt.Key.Key_Down)
    until(lambda: flick.property("contentY") > 0, "Down reads on")
    until(read_on, "Down reaches the end")
    closed()
    ask({"message": "Again", "buttons": ["OK"]})
    until(lambda: flick.property("contentY") == 0, "a new question starts at its top")


@stacked
def test_the_folder_sheet_follows_the_chip_past_the_screen_edge(look, tmp_path, monkeypatch):
    from universe_ui.screens import paths

    drives = [tmp_path / " ".join(["a folder with a long name"] * 6) / f"Drive {i:02}" for i in range(30)]
    for d in drives:
        d.mkdir(parents=True)
    monkeypatch.setattr(paths, "_mounts", lambda: [str(d) for d in drives])
    invoke(look.root, "browse", {"path": str(drives[0])}, None)
    folder = look.find("folder")
    chips = until(lambda: (c := folder.findChild(QObject, "chips")) and (c.property("count") or 0) >= 30 and c)

    def text(shown):
        return next((o for o in folder.findChildren(QObject) if o.inherits("QQuickText") and o.property("visible") and shown(str(o.property("text")))), None)

    title = until(lambda: text(lambda t: t == folder.property("title")))
    path = until(lambda: text(lambda t: t.endswith("Drive 00")))
    right = title.mapToItem(look.window.contentItem(), 0, 0).x() + title.property("implicitWidth")
    until(lambda: path.mapToItem(look.window.contentItem(), 0, 0).x() >= right, "the whole title shows, the path beside it")
    look.press(Qt.Key.Key_Up)
    look.press(Qt.Key.Key_Right, chips.property("count"))
    assert chips.property("currentIndex") == chips.property("count") - 1
    chip = chips.property("currentItem")
    until(lambda: chip.property("x") + chip.property("width") <= chips.property("contentX") + chips.property("width"), "the last chip is in view")


def test_a_long_journal_paragraph_stops_above_the_hint_bar(api):
    look = Look(api, "reprise", 1280, 800)
    game = until(lambda: look.home().property("currentGame"))
    page = look.open("pages/RecordingsPage.qml", {"game": game, "session": ""})
    paragraph = page.findChild(QQuickItem, "journalParagraph")
    until(lambda: paragraph.property("text"), "the fixture recording has an entry")
    paragraph.setProperty("text", "A paragraph that keeps going. " * 60)
    until(lambda: paragraph.property("implicitHeight") > paragraph.property("height"), "the long paragraph is cut")
    bottom = paragraph.mapToItem(look.window.contentItem(), 0, paragraph.property("height")).y()
    hint = page.findChild(QQuickItem, "hintBar")
    assert bottom <= hint.mapToItem(look.window.contentItem(), 0, 0).y()
    look.root.closeSub()
    look.close()


def test_the_power_menu_lists_what_logind_would_do(fake):
    from universe_ui.api import System

    fake.core.power_list = ["reboot"]
    system = System(fake)
    assert until(lambda: system.actions) == ["reboot"]


@reprise
def test_reprise_about_shows_the_build(look, fake):
    page = look.settings("quit")
    until(lambda: page.property("sectionId") == "about", "Quit lives in About now")
    content = read(page, "content")
    keys = [r["key"] for r in content["rows"]]
    assert keys == ["version", "changelog", "setup", "power"]
    assert content["rows"][0]["display"] == fake.version()
    assert page.property("acceptLabel") == "", "nothing to select on the version"
    assert [s["id"] for s in read(page, "sections")][-4:] == ["sound", "storage", "doctor", "about"], "no Search, Updates or Quit section"
    power = keys.index("power")
    invoke(page, "activate", power, content["rows"][power])
    confirm = look.find("confirm")
    until(
        lambda: confirm.property("open") is True and [i["action"] for i in read(confirm, "items")] == [i["action"] for i in call(look.root, "powerItems")],
        "the row opens the menu B held opens",
    )


@reprise
def test_the_reprise_game_menu_groups_its_rows_and_hides_the_media_a_game_has_none_of(look, api, fake):
    def actions():
        return [i["action"] for i in read(menu, "items")]

    def depth():
        return len(read(menu, "stack") or [])

    menu = look.find("gameMenu")
    page = look.home()
    until(lambda: game_id(page) == "the-technomancer")
    look.press(Qt.Key.Key_F1)
    until(lambda: menu.property("open") is True)
    items = read(menu, "items")
    assert [i["action"] for i in items] == ["play", "details", "favourite", "media", "manage"]
    assert [i.get("gap", False) for i in items] == [False, True, False, False, False], "play, then the rest"
    assert items[3]["more"] is True and items[4]["more"] is True
    look.press(Qt.Key.Key_Down, 3)
    look.press(Qt.Key.Key_Return)
    until(lambda: menu.property("open") is True and depth() == 1)
    counts = {kind: len(getattr(fake, kind)("the-technomancer")) for kind in ("screenshots", "recordings", "journal")}
    assert all(counts.values()), "the fixture game has every kind"
    assert actions() == list(counts), "the kinds the game has, no counts"
    look.press(Qt.Key.Key_Escape)
    until(lambda: depth() == 0, "B comes back to the row that opened it")
    assert menu.property("open") is True and menu.property("index") == 3 and actions()[3] == "media", "B comes back to the row that opened it"
    look.press(Qt.Key.Key_Escape)
    until(lambda: menu.property("open") is False)

    invoke(look.root, "openMenu", api.allGames.byId("mini-metro"), page.property("menuAnchor"))
    until(lambda: actions() == ["play", "details", "favourite", "manage"], "nothing to browse: no Media row")
    look.press(Qt.Key.Key_Down, 3)
    look.press(Qt.Key.Key_Return)
    until(lambda: depth() == 1 and actions() == ["settings", "artwork", "sessions", "data", "remove"])
    assert read(menu, "items")[4]["danger"] is True
    look.press(Qt.Key.Key_Down, 4)
    look.press(Qt.Key.Key_Return)
    until(lambda: depth() == 2, "the removal asks first")
    look.press(Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    until(lambda: menu.property("open") is False)
    until(lambda: api.allGames.byId("mini-metro") is None)


def game_settings(look, key):
    """The game's settings page, landed on `key`."""
    page = look.open("pages/GameSettingsPage.qml", {**look.game("the-technomancer"), "key": key})
    until(lambda: current_row(page).get("key") == key, f"landed on {key}")
    settle(look.window)
    return page


def sheet_open(page, name):
    """Whether the value editor's `name` sheet is up: they load on first use."""
    found = page.findChild(QObject, name)
    return found is not None and found.property("open") is True


@reprise
def test_the_game_settings_page_applies_a_value_to_all_games(look, fake):
    page = game_settings(look, "launch.ntsync")
    messages = record(page.message)
    look.press(Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"]["ntsync"] is False and page.property("canPromote") is True)
    look.press(Qt.Key.Key_F1)
    look.press(Qt.Key.Key_Down, [i["action"] for i in read(page, "moreItems")].index("promote"))
    look.press(Qt.Key.Key_Return)
    until(lambda: fake.config()["set"]["launch"]["ntsync"] is False and own(fake, "ntsync") is None)
    until(lambda: current_row(page)["origin"] == "global" and page.property("canPromote") is False)
    assert len(messages) == 1, "the form's word on what came of it, once"


def test_game_settings_land_a_hit_behind_advanced_and_edit_it(look, api, fake):
    form = api.screens.gameSettings
    page = game_settings(look, "launch.ntsync")
    assert form.showAdvanced is True, "an advanced row: Advanced comes on"
    sections = read(page, "sections")
    look.press(Qt.Key.Key_Return)
    until(
        lambda: fake.game("the-technomancer")["launch"]["ntsync"] is False and current_row(page)["origin"] == "game",
        "changing the value is what sets it on the game",
    )
    look.press(Qt.Key.Key_I)
    until(lambda: own(fake, "ntsync") is None and current_row(page)["origin"] != "game", "X drops the game's own value")
    look.press(Qt.Key.Key_F)
    until(lambda: form.showAdvanced is False and read(page, "sections") == sections, "Y: the rows go, the cards stay")


@reprise
def test_the_game_settings_page_shows_an_advanced_change_with_advanced_off(look, api, fake):
    page = game_settings(look, "launch.proton")
    form = api.screens.gameSettings
    proton = page.findChild(QObject, "cardSections").property("section")
    look.press(Qt.Key.Key_Down, 3)
    until(lambda: current_row(page)["key"] == "launch.prefix", "the game's own prefix sits under the Proton card's rows")
    assert form.showAdvanced is False
    look.press(Qt.Key.Key_I)
    until(
        lambda: own(fake, "prefix") is None and current_row(page)["key"] == "launch.proton",
        "reset, the row goes back behind Advanced and the cursor to the card's first row",
    )
    assert form.showAdvanced is False and page.findChild(QObject, "cardSections").property("section") == proton


@reprise
def test_the_game_settings_page_adds_a_variable_from_one_sheet(look, fake):
    def type_text(text):
        for ch in text:
            QTest.keyClick(look.window, ch)

    page = game_settings(look, "launch.env")
    assert current_row(page)["map"] is True, "the hit lands on the row that adds a variable"
    look.press(Qt.Key.Key_Return)
    until(lambda: sheet_open(page, "textSheet"))
    sheet = page.findChild(QObject, "textSheet")
    assert sheet.property("pair") is True and sheet.property("typing") == 0, "one sheet, two fields: the name first"
    type_text("DXVK_HUD")
    look.press(Qt.Key.Key_Return)
    until(lambda: sheet.property("typing") == 1, "then the value")
    type_text("fps")
    look.press(Qt.Key.Key_Return)
    until(lambda: [h["glyph"] for h in read(page, "hints")] == ["A", "Y", "B"], "then which games it is for: A this one, Y all")
    look.press(Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"].get("env") == {"DXVK_HUD": "fps"})
    until(lambda: current_row(page)["key"] == "launch.env.DXVK_HUD", "the new variable is a row of its own, the cursor on it")
    assert current_row(page)["origin"] == "game"
    look.press(Qt.Key.Key_I)
    until(lambda: fake.game("the-technomancer")["launch"].get("env", {}) == {}, "X removes it")


@reprise
def test_a_path_row_is_typed_first_under_a_keyboard(look, api, fake):
    from PySide6.QtCore import QCoreApplication

    from universe_ui import gamepad

    warnings = record(look.engine.warnings)
    page = game_settings(look, "launch.exe")
    assert current_row(page)["type"] == "path"
    gamepad.post_key(Qt.Key.Key_Return, True, window=look.window)
    gamepad.post_key(Qt.Key.Key_Return, False, window=look.window)
    QCoreApplication.sendPostedEvents()
    until(lambda: sheet_open(page, "pathSheet"), "under a pad the folders come first")
    assert api.keys.mode == "pad"
    look.press(Qt.Key.Key_Escape)
    look.press(Qt.Key.Key_Return)
    until(lambda: sheet_open(page, "textSheet") and not sheet_open(page, "pathSheet"), "under a keyboard the path is typed first")
    assert api.keys.mode == "keyboard"
    look.press("2")
    look.press(Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"]["exe"] == "/mnt/games/PC/The Technomancer/TheTechnomancer.exe2", "the field held the value")
    look.press(Qt.Key.Key_Return)
    until(lambda: sheet_open(page, "textSheet"))
    look.press(Qt.Key.Key_F1)
    until(lambda: sheet_open(page, "pathSheet") and api.screens.paths.files is True, "F1: the folders, for a file")
    look.press(Qt.Key.Key_F)
    until(lambda: sheet_open(page, "textSheet") and not sheet_open(page, "pathSheet"), "Y: back to typing")
    look.press(Qt.Key.Key_Escape)
    until(lambda: not sheet_open(page, "textSheet"))
    assert warnings == []


def test_the_sheets_type_the_physical_keyboards_letters(fake, tmp_path, monkeypatch):
    from PySide6.QtCore import QCoreApplication

    from universe_ui import gamepad
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    def press(key=Qt.Key.Key_Return):
        gamepad.post_key(key, True, window=look.window)
        gamepad.post_key(key, False, window=look.window)
        QCoreApplication.sendPostedEvents()

    monkeypatch.setenv("XKB_DEFAULT_LAYOUT", "fr")
    monkeypatch.setenv("XKB_DEFAULT_VARIANT", "")
    exe = tmp_path / "The Technomancer" / "TheTechnomancer.exe"
    exe.parent.mkdir()
    fake.set("the-technomancer", "launch.exe", str(exe))
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    look = Look(api, "reprise")
    page = game_settings(look, "launch.exe")
    press()
    until(lambda: sheet_open(page, "pathSheet"))
    look.press(Qt.Key.Key_F)
    until(lambda: sheet_open(page, "textSheet") and page.findChild(QObject, "textSheet").property("browsable") is True, "Y on the folders: the keyboard sheet")
    press()
    look.press(Qt.Key.Key_Down, 4)
    press()
    look.press(Qt.Key.Key_Up, 4)
    press()
    press(Qt.Key.Key_F)
    until(
        lambda: fake.game("the-technomancer")["launch"]["exe"] == os.path.dirname(exe) + "a&",
        "the first letter key is A, and ⇧ on the number row types the layout's own level",
    )

    look.switch("switch2")
    look.open("pages/SettingsSearchPage.qml")
    settle(look.window)
    press()
    look.press(Qt.Key.Key_Down, 3)
    press()
    look.press(Qt.Key.Key_Up, 4)
    press()
    press()
    until(lambda: api.screens.search.query == "a&1", "the Switch's ⇧ types one key")
    look.close()
    api.shutdown()


@switch2
def test_the_switch2_forms_share_the_sidebar_and_y(look, api):
    def glyphs(page):
        return [h["glyph"] for h in read(page, "hints")]

    def sections(page):
        return [s["label"] for s in read(page, "sections")]

    warnings = record(look.engine.warnings)
    form = api.screens.runner
    page = look.open("pages/FormPage.qml", {"runner": "proton"})
    until(lambda: len(sections(page)) == 4)
    before = sections(page)
    assert form.showAdvanced is False
    look.press(Qt.Key.Key_F)
    until(lambda: form.showAdvanced is True)
    assert sections(page) == before, "Y: the sidebar stays"
    look.press(Qt.Key.Key_Right)
    look.press(Qt.Key.Key_Down, 3)
    row = until(lambda: (row := read(page, "currentRow")) and row["key"] == "gamescope" and row)
    assert row["origin"] == "default" and "X" in glyphs(page), "config.toml leaves gamescope to its default"
    look.press(Qt.Key.Key_Return)
    until(lambda: form.rows[row["form"]]["origin"] == "runner", "toggling the inherited switch sets it on the runner")
    look.press(Qt.Key.Key_I)
    until(lambda: form.rows[row["form"]]["origin"] == "default", "X clears it back")
    look.press(Qt.Key.Key_Escape, 2)
    page = look.open("pages/FormPage.qml", {"source": "gog"})
    until(lambda: len(sections(page)) == 3 and "Y" in glyphs(page))
    look.press(Qt.Key.Key_Escape)
    look.settings("launch")
    look.press(Qt.Key.Key_F)
    until(lambda: api.screens.launch.showAdvanced is True, "Y opens the Launch section's advanced rows")
    assert warnings == []


# The fixture's pad supply hangs off event30: the fake pad's node, so its glyph goes green while that pad is current.
def test_the_badge_tints_the_current_pad(api):
    from PySide6.QtGui import QColor

    from universe_ui.screens.controller import FakeWatcher

    look = Look(api, "reprise")
    controller = api.screens.controller
    controller.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    controller.start(watcher)
    badge = until(lambda: next((c for c in look.window.findChildren(QObject) if c.property("currentTint") is not None), None))

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
    look.close()


def test_the_sound_section_plays_through_the_output_picked(look, fake):
    def current():
        return next(o["label"] for o in fake.core.outputs_list if o["current"])

    def rows():
        content = read(page, "content") or []
        return content["rows"] if isinstance(content, dict) else content

    page = look.settings("sound")
    until(lambda: current() == "Speakers" and any(r.get("label") == "Headphones" for r in rows()))
    index = next(i for i, r in enumerate(rows()) if r.get("label") == "Headphones")
    invoke(page, "activate", index, rows()[index])
    until(lambda: current() == "Headphones", "the output picked plays")


@pytest.mark.parametrize("name", LOOKS)
def test_in_the_universe_session_the_way_out_logs_out(universe_session, api, name):
    until(lambda: api.system.actions)
    look = Look(api, name)
    keys = [i.get("action", i.get("act")) for i in call(look.root, "powerItems")]
    way_out = "logout" if universe_session else "quit"
    assert way_out in keys and "power_off" in keys, keys
    assert ("quit" if universe_session else "logout") not in keys, "one way out of the launcher"
    look.close()


def test_a_runner_pages_builds_card_installs_after_asking(look, api, fake):
    components = api.screens.components
    components.load()
    until(lambda: not components.busy)
    page = look.open("pages/FormPage.qml", {"runner": "rpcs3"})
    form = api.screens.runner
    index = until(lambda: next((i for i, r in enumerate(form.rows) if r.get("component") == "rpcs3"), None), "the Builds card")
    assert sum(index in g["rows"] for g in form.groups) == 1, "the build row sits in one card"
    settle(look.window)
    finished = record(fake.jobFinished)
    invoke(page, "activate", index, form.rows[index])
    owner = page if look.name == "reprise" else look.root
    until(lambda: owner.findChild(QObject, "menu" if look.name == "reprise" else MENU[look.name]).property("open") is True, "A opens the build's options")
    look.press(Qt.Key.Key_Return)
    until(lambda: owner.findChild(QObject, "dialog").property("open") is True, "the install asks first: its size, the room left")
    assert components.job is None
    look.press(Qt.Key.Key_Return)
    until(lambda: components.job is not None and components.job["component"] == "rpcs3", "Install is the default")
    until(lambda: finished)
    until(lambda: not components.busy)
    until(lambda: not next(r for r in form.rows if r.get("component") == "rpcs3")["accent"], "in: nothing left to do")
