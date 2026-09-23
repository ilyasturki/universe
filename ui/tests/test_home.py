import os
from types import SimpleNamespace

from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from test_render import lit_fraction, render

from conftest import pump, wait_for
from universe_ui import host


def stop(api):
    api.universe.stop(api.universe.currentSession["session_id"])
    wait_for(api.universe.sessionEnded, 5000)
    pump(50)


def test_a_shot_from_the_pad_or_the_dock_cues_the_same_way(api, fake):
    home = api.home
    taken = []
    home.screenshotTaken.connect(taken.append)
    api.screens.controller._on_event({"event": "screenshot", "path": "/tmp/x.png"})
    assert taken == ["/tmp/x.png"], "the watcher's event reaches the theme through api.home"
    api.screens.controller._on_event({"event": "screenshot", "path": ""})
    assert taken == ["/tmp/x.png", ""], "a failed shot is reported too, silently"
    home.screenshot()
    for _ in range(50):
        pump(50)
        if len(taken) == 3:
            break
    assert len(taken) == 3 and taken[2].endswith("screenshot.png"), "the dock's shot goes through the same signal"


def test_home_flips_between_the_game_and_the_launcher(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    home = api.home
    assert home.shown == "launcher" and not home.open
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    assert home.shown == "game", "gamescope's root says the game's window is up"
    assert home.pauseOnHome and not home.paused and fake.core.frozen is False, "on by default, and the game runs while it is shown"
    home.toLauncher()
    pump(250)
    assert home.shown == "launcher" and home.frame != "" and fake.core.game_shown is True, "the frame is taken and offered to the theme before the swap"
    pump(800)
    assert fake.core.game_shown is False, "a theme that never says it has painted the frame still gets the swap"
    assert home.paused and fake.core.frozen is True, "the launcher over the game: frozen, so the pad drives the menu alone"
    home.changed.connect(home.covered)
    home.toGame()
    pump(400)
    assert home.shown == "game" and fake.core.game_shown is True
    assert not home.paused and fake.core.frozen is False
    home.setPauseOnHome(False)
    home.toLauncher()
    pump(400)
    assert home.shown == "launcher" and fake.core.frozen is False, "off for this game: it keeps running behind the launcher"
    home.setPauseOnHome(True)
    pump(50)
    assert fake.core.frozen is True, "turned on while the launcher covers it: frozen now"
    home.toGame()
    pump(400)
    assert fake.core.frozen is False
    home.openDock()
    pump(400)
    assert not home.open and home.shown == "launcher", "no overlay window: HOME goes home instead"
    assert fake.core.frozen is True
    stop(api)
    assert home.shown == "launcher" and not home.paused


def test_on_the_desktop_the_game_is_shown_once_its_window_maps(api, fake):
    home = api.home
    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))
    fake.launch("mirrors-edge", "")
    pump(50)
    assert home.shown == "launcher"
    wait_for(fake.sessionShown, 3000)
    pump(400)
    assert home.shown == "game" and errors == [], "nothing polls gamescope's root outside of it"
    home.toLauncher()
    pump(50)
    assert home.shown == "launcher"
    stop(api)


def test_the_dock_pauses_on_home_and_thaws_on_the_release(api, fake):
    home = api.home
    assert home.attachOverlay(object()) is False, "mapped only inside gamescope"
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    home.setPauseOnHome(False)
    home.setPauseOnHome(True)
    assert home.pauseOnHome and fake.game("mirrors-edge")["launch"]["pause_on_home"] is True
    home.guide(True)
    home.openDock()
    pump(50)
    assert home.open and home.paused and fake.core.frozen is True
    assert api.screens.controller._suspended is True and api.screens.controller._docked is True, (
        "the dock has the pad from the moment it opens; the docked macros still fire"
    )
    home.closeDock()
    home.dockClosed()
    pump(50)
    assert not home.open and home.paused and fake.core.frozen is True, "HOME still held: the game stays frozen until the release"
    assert api.screens.controller._suspended is False
    home.guide(False)
    pump(50)
    assert not home.paused and fake.core.frozen is False
    home.setPauseOnHome(False)
    home.guide(True)
    home.openDock()
    home.guide(False)
    assert home.open and not home.paused and api.screens.controller._suspended is True, "released with the dock up: the macros stop"
    home.closeDock()
    home.dockClosed()
    assert api.screens.controller._suspended is False
    stop(api)
    assert not home.open and home.shown == "launcher"


def test_the_dock_over_the_game_takes_the_pad_back(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    assert home.attachOverlay(Overlay()) is True
    assert not home.padCovered, "the launcher alone reads the pad"
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    assert home.shown == "game" and home.padCovered, "the game on screen has the pad"
    home.openDock()
    pump(50)
    assert home.open and not home.padCovered, "the dock over the game is driven by the pad"
    home.closeDock()
    assert home.padCovered, "closing: the presses are the game's again"
    home.dockClosed()
    stop(api)
    assert not home.padCovered


def test_a_guide_hold_from_the_game_goes_home(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    presses = []

    def theme_press():
        # What Reprise does with a press: home from the game opens its dock, home from the launcher resumes.
        presses.append(home.shown)
        if home.shown == "launcher":
            home.toGame()

    home.pressed.connect(theme_press)
    home.guide(True)
    pump(200)
    home.guide(False)
    pump(300)
    assert presses == ["game"] and home.shown == "game", "a tap is the theme's press alone"
    home.guide(True)
    pump(800)
    assert home.shown == "launcher" and fake.core.game_shown is False, "held past hold_ms: the launcher comes up before the release"
    home.guide(False)
    pump(50)
    assert home.shown == "launcher" and presses == ["game", "game"]
    home.guide(True)
    pump(800)
    home.guide(False)
    pump(400)
    assert presses == ["game", "game", "launcher"] and home.shown == "game", "from the launcher a press resumes, and holding it there does not bounce back"
    home.guide(True)
    home.openDock()
    pump(50)
    assert home.shown == "launcher", "no overlay: the press itself went home"
    stop(api)


def test_the_press_takes_the_frame_the_flip_waits_on(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "FRAME_S", 0.5)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    home.pressed.connect(lambda: home.toGame() if home.shown == "launcher" else None)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    home.guide(True)
    pump(200)
    home.guide(False)
    pump(500)
    assert fake.core.frames == 1 and home.shown == "game" and home.frame == "", "a tap asks for the frame; landing late, it flips nothing"
    home.guide(True)
    pump(700)
    assert home.shown == "launcher" and home.frame != "" and fake.core.frames == 2, "the hold flips with the frame its press took, at hold_ms"
    home.guide(False)
    pump(50)
    home.toGame()
    pump(300)
    home.guide(True)
    pump(100)
    home.toLauncher()
    pump(200)
    assert home.shown == "game" and fake.core.frames == 3, "asked while the press's frame is on its way: the flip waits for it, and asks for no other"
    pump(400)
    assert home.shown == "launcher" and fake.core.game_shown is False
    home.guide(False)
    home.setPauseOnHome(False)
    home.toGame()
    pump(300)
    home.guide(True)
    pump(200)
    home.guide(False)
    pump(1500)
    frame = home.frame
    home.toLauncher()
    pump(100)
    assert home.shown == "game" and fake.core.frames == 5, "a game that ran on under the dock: the press's frame is stale, a new one is taken"
    pump(600)
    assert home.shown == "launcher" and home.frame != frame
    home.toGame()
    pump(300)
    monkeypatch.setattr(fake.core, "nest_frame", lambda: None)
    home.toLauncher()
    pump(300)
    assert home.shown == "launcher" and home.frame == "" and fake.core.game_shown is False, "no frame in time: the swap goes ahead, nothing to zoom"
    home.toGame()
    pump(300)
    monkeypatch.undo()
    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "FRAME_S", 0.5)
    assert api.theme.set("switch2")
    home.guide(True)
    home.toLauncher()
    pump(100)
    assert home.shown == "launcher" and fake.core.game_shown is False and fake.core.frames == 5, "a look that paints no frame waits for none"
    home.guide(False)
    stop(api)


def test_a_theme_that_covers_at_once_gets_the_swap_at_once(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    seen = {"shown": home.shown}

    def theme_changed():
        # As the Switch 2 look: one answer per game-to-launcher edge, from the first `changed` it sees.
        if home.shown == "launcher" and seen["shown"] == "game":
            home.covered()
        seen["shown"] = home.shown

    home.changed.connect(theme_changed)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    home.toLauncher()
    pump(120)
    assert home.shown == "launcher" and home.paused and fake.core.game_shown is False, "swapped well inside COVER_MS"
    stop(api)


def test_a_hold_that_flips_leaves_nothing_to_thaw_on_the_release(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    assert home.attachOverlay(SimpleNamespace(winId=lambda: 1, show=lambda: None)) is True
    home.guide(True)
    home.openDock()
    home.closeDock()
    home.dockClosed()
    pump(50)
    assert home.paused and fake.core.frozen is True, "closed while HOME is held: frozen until the release"
    pump(700)
    assert home.shown == "launcher" and home.paused, "the hold went home meanwhile"
    home.guide(False)
    pump(50)
    assert home.paused and fake.core.frozen is True, "the release thaws nothing under the launcher"
    stop(api)


def test_quitting_from_the_game_brings_the_launcher_up_first(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    titles, ended, stops = [], [], []
    home.stopping.connect(titles.append)
    api.universe.sessionEnded.connect(lambda *a: ended.append(a))
    real_stop = fake.core.stop
    monkeypatch.setattr(fake.core, "stop", lambda sid: (stops.append((fake.core.game_shown, fake.core.frozen)), real_stop(sid)))
    home.stop()
    home.stop()
    assert titles == ["Mirror's Edge"], "a second Quit while one is under way is nothing"
    if not ended:
        wait_for(api.universe.sessionEnded, 5000)
    pump(50)
    assert stops == [(False, False)], "the launcher is up, and the game never frozen, when it is asked to quit: the SIGTERM has to land"
    assert len(ended) == 1 and home.shown == "launcher" and not home.paused


def test_the_hud_is_the_games_key_and_the_reload_key_waits_for_the_thaw(api, fake):
    from universe_ui.screens.controller import FakeWatcher

    def hud_settles(on):
        for _ in range(60):
            if fake.core.hud_shown is on:
                return
            pump(50)

    home = api.home
    home.attachOverlay(object())
    watcher = FakeWatcher("dualsense-edge")
    api.screens.controller.restart_ms = 0
    api.screens.controller.start(watcher)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    assert home.launchValue("mangohud") == "false" and fake.core.hud_shown is False, "off by default, hidden at launch"
    home.setLaunchValue("mangohud", "true")
    hud_settles(True)
    assert fake.game("mirrors-edge")["launch"]["mangohud"] is True and fake.core.hud_shown is True, "written as the game's own key, shown in the game"
    assert home.launchValue("mangohud") == "true"
    home.setLaunchValue("mangohud", "false")
    hud_settles(False)
    assert fake.game("mirrors-edge")["launch"]["mangohud"] is False and fake.core.hud_shown is False
    assert not any(c.get("action") == "keys" for c in watcher.commands), "no key typed for the HUD"

    home.setPauseOnHome(True)
    home.openDock()
    pump(50)
    assert home.paused and fake.core.frozen is True
    home.setLaunchValue("fps_limit", "60")
    home.setLaunchValue("fps_limit", "30")
    pump(100)
    assert not any(c.get("action") == "keys" for c in watcher.commands), "a frozen game reads no key"
    home.closeDock()
    home.dockClosed()
    pump(100)
    assert not home.paused and fake.core.frozen is False
    typed = [c for c in watcher.commands if c.get("action") == "keys"]
    assert typed == [{"cmd": "run", "action": "keys", "keys": "Shift_L+F4"}], "the reload, once, after the thaw"
    stop(api)


def test_the_dock_over_a_loading_game_is_home_and_quit_and_grows_once_the_window_is_up(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "WINDOW_S", 1.2)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.attachOverlay(Overlay())
    assert not home.loading
    fake.launch("mirrors-edge", "")
    wait_for(fake.launched, 3000)
    pump(50)
    assert home.loading and home.shown == "launcher", "launched, no window yet: the poster holds"
    home.openDock()
    pump(50)
    assert home.open and not home.paused and fake.core.frozen is False, "the dock over the poster freezes nothing: the game is still starting"
    home.setPauseOnHome(True)
    assert fake.core.frozen is False
    wait_for(fake.sessionShown, 3000)
    pump(300)
    assert not home.loading and home.open, "the window is up: the dock stays, grown to the full one"
    assert home.paused and fake.core.frozen is True, "…and the game freezes under it, as pause on HOME asks"
    home.closeDock()
    home.dockClosed()
    pump(100)
    assert not home.paused and fake.core.frozen is False
    stop(api)
    assert not home.loading


def test_home_from_the_dock_over_a_loading_game_takes_no_frame_and_freezes_nothing(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "WINDOW_S", 1.2)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.attachOverlay(Overlay())
    fake.launch("mirrors-edge", "")
    wait_for(fake.launched, 3000)
    pump(50)
    home.openDock()
    pump(50)
    home.toLauncher()
    pump(100)
    assert not home.open and home.flipped and home.frame == "" and fake.core.frames == 0, "nothing painted yet: no frame asked, the theme drops its poster"
    assert fake.core.frozen is False
    wait_for(fake.sessionShown, 3000)
    pump(300)
    assert not home.loading and not home.flipped, "the game mapped over the launcher on its own"
    stop(api)


def test_quit_from_the_dock_over_a_loading_game_stops_it_through_the_launcher(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "WINDOW_S", 2.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.attachOverlay(Overlay())
    quitting = []
    home.stopping.connect(quitting.append)
    fake.launch("mirrors-edge", "")
    wait_for(fake.launched, 3000)
    pump(50)
    home.openDock()
    pump(50)
    home.stop()
    pump(100)
    assert quitting == ["Mirror's Edge"] and not home.open and home.flipped and fake.core.frames == 0 and fake.core.frozen is False, (
        "the launcher comes up, no frame, nothing frozen"
    )
    assert wait_for(fake.sessionEnded, 5000) is not None, "the unit was stopped"
    pump(100)
    assert not home.loading and not home.flipped and home.shown == "launcher"


def test_the_dock_opens_over_the_poster_before_the_core_has_made_the_session(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "START_S", 1.0)
    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "WINDOW_S", 1.2)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.attachOverlay(Overlay())
    fake.launch("mirrors-edge", "")
    pump(100)
    assert fake.currentSession is None and home.pending == {"id": "mirrors-edge", "title": "Mirror's Edge"} and home.loading
    home.openDock()
    pump(50)
    assert home.open and not home.paused and fake.core.frozen is False, "HOME answers while the pads are still being taken"
    wait_for(fake.launched, 3000)
    pump(50)
    assert home.pending is None and home.open and home.loading, "the session made: the same reduced dock stays up"
    wait_for(fake.sessionShown, 3000)
    pump(300)
    assert not home.loading and home.open and home.paused
    home.closeDock()
    home.dockClosed()
    pump(100)
    stop(api)


def test_quit_before_the_session_is_made_stops_it_once_the_core_hands_it_back(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "START_S", 1.0)
    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "WINDOW_S", 2.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.attachOverlay(Overlay())
    quitting = []
    home.stopping.connect(quitting.append)
    fake.launch("mirrors-edge", "")
    pump(100)
    home.openDock()
    pump(50)
    home.stop()
    pump(100)
    assert quitting == ["Mirror's Edge"] and not home.open and home.flipped and fake.core.frames == 0
    assert wait_for(fake.sessionEnded, 5000) is not None, "stopped as soon as it existed"
    pump(100)
    assert home.pending is None and not home.loading and not home.flipped and home.shown == "launcher"


def test_a_launch_that_fails_before_its_session_takes_the_dock_down(api, fake, monkeypatch):
    from universe_ui import fake_core

    class Overlay:
        def winId(self):
            return 7

        def show(self):
            pass

    monkeypatch.setattr(fake_core, "START_S", 0.5)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.attachOverlay(Overlay())
    fake.launch("no-such-game", "")
    pump(50)
    home.openDock()
    pump(50)
    assert home.open
    assert wait_for(fake.launchFailed, 3000) is not None
    pump(50)
    assert home.pending is None and not home.open and not home.loading


def test_sharpness_reaches_gamescope_with_the_games_filter(api, fake, monkeypatch):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    assert home.launchValue("gamescope_sharpness") == "" and home.launchChoices("gamescope_sharpness") == ["0", "2", "5", "10", "15", "20"]
    home.setLaunchValue("gamescope_filter", "fsr")
    assert fake.core.filter == ("fsr", None), "no sharpness set: the card goes, gamescope's default holds"
    home.setLaunchValue("gamescope_sharpness", "5")
    assert str(fake.game("mirrors-edge")["launch"]["gamescope_sharpness"]) == "5", "written as the game's own key"
    assert fake.core.filter == ("fsr", 5) and home.launchValue("gamescope_sharpness") == "5", "applied with the filter the game has"
    home.setLaunchValue("gamescope_sharpness", "")
    assert fake.core.filter == ("fsr", None)
    stop(api)


def covered_fraction(image):
    small = image.scaled(96, 54)
    covered = sum(1 for y in range(small.height()) for x in range(small.width()) if small.pixelColor(x, y).alpha() > 10)
    return covered / (small.width() * small.height())


def key(window, k):
    QTest.keyClick(window, k)
    pump(50)


def test_the_dock_renders_over_a_running_game(api, fake, tmp_path, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    engine, window = render(api)
    overlay = host.create_overlay(engine, window.size())
    assert overlay is not None
    api.home.attachOverlay(overlay)
    overlay.show()
    pump(200)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    api.home.openDock()
    overlay.requestActivate()
    pump(500)
    dock = overlay.property("contentItem").childItems()[0].property("item")
    assert dock is not None and dock.property("open") is True
    image = overlay.grabWindow()
    assert 0.3 < covered_fraction(image) < 0.6, "the band covers the lower part of the frame"
    assert image.pixelColor(4, 4).alpha() == 0, "the top of the frame stays clear"
    assert lit_fraction(image, "#000000") > 0.01, "the card and the buttons are drawn on it"
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Right)
    assert dock.property("index") == 2
    key(overlay, Qt.Key.Key_Return)
    assert dock.property("opened") is True, "Game opens its card"
    assert api.home.pauseOnHome is True
    for _ in range(3):
        key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Return)
    assert api.home.pauseOnHome is False, "past Details, Journal and Recordings sits Pause on HOME, on by default: A turns it off"
    key(overlay, Qt.Key.Key_Escape)
    assert dock.property("opened") is False
    key(overlay, Qt.Key.Key_Escape)
    pump(400)
    assert api.home.open is False, "B closes the dock"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        image.save(str(tmp_path / "dock.png"))

    root = window.property("contentItem").childItems()[0].property("item")
    api.home.openDock()
    overlay.requestActivate()
    pump(300)
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Return)
    pump(600)
    assert api.home.shown == "launcher" and api.home.open is False and root.property("detailOpen") is False, "Home from the dock: the launcher, nothing opened"
    api.home.toGame()
    pump(400)
    root.goToTab(2)
    api.home.openDock()
    overlay.requestActivate()
    pump(300)
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Return)
    key(overlay, Qt.Key.Key_Return)
    pump(600)
    assert api.home.shown == "launcher" and api.home.open is False, "Details in the Game card goes home"
    assert root.property("tabIndex") == 0 and root.property("detailOpen") is True, "…lands on Home and opens the playing game's details there"
    assert api.home.takeLanding() == "", "taken once"
    api.home.toGame()
    pump(400)
    stop(api)
    window.close()
    overlay.close()
    pump(50)


def test_the_docks_output_row_switches_once_the_cursor_rests(api, fake, tmp_path, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    switched = []
    real_set = fake.core.set_output
    monkeypatch.setattr(fake.core, "set_output", lambda ident: (switched.append(ident), real_set(ident))[1])
    engine, window = render(api)
    overlay = host.create_overlay(engine, window.size())
    api.home.attachOverlay(overlay)
    overlay.show()
    pump(200)
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    api.home.openDock()
    overlay.requestActivate()
    wait_for(api.home.outputsChanged, 3000)
    pump(300)
    dock = overlay.property("contentItem").childItems()[0].property("item")
    dock.setProperty("index", [b["id"] for b in dock.property("buttons").toVariant()].index("sound"))
    key(overlay, Qt.Key.Key_Return)
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Down)
    assert dock.property("target").toVariant()["id"] == "output" and dock.property("vals").toVariant()["output"].endswith("speaker")
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Right)
    assert dock.property("vals").toVariant()["output"].endswith("hdmi-output-0"), "the row shows the pick at once"
    assert [o["label"] for o in api.home.outputs if o["current"]] == ["Speakers"], "nothing switched while stepping"
    wait_for(api.home.outputsChanged, 3000)
    assert [o["label"] for o in api.home.outputs if o["current"]] == ["HDMI / DisplayPort"]
    assert len(switched) == 1 and switched[0].endswith("hdmi-output-0"), "one switch, to where the cursor rested"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(str(tmp_path / "dock-output.png"))
    stop(api)
    window.close()
    overlay.close()
    pump(50)


def test_home_over_the_poster_raises_home_and_quit_and_home_drops_the_poster(api, fake, tmp_path, monkeypatch):
    from PySide6.QtCore import Q_ARG, QMetaObject, QObject

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setattr(fake_core, "WINDOW_S", 2.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    engine, window = render(api, activate=True)
    overlay = host.create_overlay(engine, window.size())
    api.home.attachOverlay(overlay)
    overlay.show()
    pump(200)
    root = window.property("contentItem").childItems()[0].property("item")
    poster = window.findChild(QObject, "launchOverlay")
    dock = overlay.property("contentItem").childItems()[0].property("item")
    QMetaObject.invokeMethod(root, "launchGame", Q_ARG("QVariant", api.allGames.byId("control")))
    api.home.pressed.emit()
    assert api.home.open is False, "HOME before the session exists is swallowed with the other keys"
    wait_for(fake.sessionStarted, 3000)
    pump(100)
    assert poster.property("waiting") is True and api.home.loading is True
    api.home.pressed.emit()
    overlay.requestActivate()
    pump(300)
    assert api.home.open is True and dock.property("loading") is True
    assert [b["id"] for b in dock.property("buttons").toVariant()] == ["home", "quit"], "over the poster: Home and Quit alone"
    assert api.home.paused is False and fake.core.frozen is False
    image = overlay.grabWindow()
    assert 0.3 < covered_fraction(image) < 0.6 and lit_fraction(image, "#000000") > 0.01, "drawn like the full one"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        image.save(str(tmp_path / "dock-loading.png"))
    key(overlay, Qt.Key.Key_Down)
    assert dock.findChild(QObject, "dockShots").property("open") is False, "no shots while loading"
    api.home.pressed.emit()
    pump(400)
    assert api.home.open is False, "a second press closes it, the poster still holds"
    assert poster.property("waiting") is True
    api.home.pressed.emit()
    overlay.requestActivate()
    pump(300)
    key(overlay, Qt.Key.Key_Return)
    pump(600)
    assert api.home.open is False and poster.property("waiting") is False and fake.core.frames == 0, "Home: the poster goes, no frame was asked for"
    assert fake.core.frozen is False and root.property("playingId") == "control"
    wait_for(fake.sessionShown, 4000)
    pump(300)
    assert api.home.loading is False and api.home.shown == "game"
    api.home.openDock()
    overlay.requestActivate()
    pump(300)
    assert [b["id"] for b in dock.property("buttons").toVariant()][:3] == ["resume", "home", "game"], "the full dock once the game is up"
    assert dock.property("index") == 0
    key(overlay, Qt.Key.Key_Escape)
    pump(400)
    stop(api)
    window.close()
    overlay.close()
    pump(50)


def test_the_dock_lists_the_sessions_shots_and_trashes_one(api, fake, tmp_path, monkeypatch):
    from PySide6.QtCore import QObject

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    engine, window = render(api)
    overlay = host.create_overlay(engine, window.size())
    api.home.attachOverlay(overlay)
    overlay.show()
    pump(200)
    fake.launch("the-technomancer", "")
    wait_for(fake.sessionShown, 3000)
    pump(300)
    earlier = len(fake.screenshots("the-technomancer"))
    assert earlier > 0, "the fixture seeds shots on past sessions"
    api.home.screenshot()
    wait_for(api.home.screenshotTaken, 3000)
    pump(100)
    api.home.openDock()
    overlay.requestActivate()
    pump(300)
    dock = overlay.property("contentItem").childItems()[0].property("item")
    panel = overlay.findChild(QObject, "dockShots")
    assert panel is not None and panel.property("open") is False
    key(overlay, Qt.Key.Key_Down)
    pump(500)
    assert panel.property("open") is True, "▼ from the row raises the screenshots over the game"
    assert api.screens.shots.gameId == "the-technomancer"
    assert panel.property("mine") == 1 and panel.property("count") == earlier + 1, "the shot just taken sits under THIS SESSION, the seeded ones under EARLIER"
    assert dock.property("opened") is False
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(str(tmp_path / "dock-shots.png"))
    key(overlay, Qt.Key.Key_F)
    assert panel.property("busy") is True, "Y asks before trashing"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(str(tmp_path / "dock-shots-confirm.png"))
    key(overlay, Qt.Key.Key_Escape)
    assert panel.property("busy") is False and panel.property("count") == earlier + 1, "B keeps it"
    key(overlay, Qt.Key.Key_F)
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Return)
    for _ in range(40):
        pump(50)
        if panel.property("count") == earlier:
            break
    assert panel.property("mine") == 0 and panel.property("count") == earlier, "trashed: the list follows the directory"
    assert len(fake.screenshots("the-technomancer")) == earlier
    key(overlay, Qt.Key.Key_Up)
    pump(500)
    assert panel.property("open") is False and api.home.open is True, "▲ past the top row lowers the panel onto the dock"
    key(overlay, Qt.Key.Key_Escape)
    pump(400)
    assert api.home.open is False
    stop(api)
    window.close()
    overlay.close()
    pump(50)


def red_fraction(image):
    small = image.scaled(96, 54)
    red = sum(1 for y in range(small.height()) for x in range(small.width()) if small.pixelColor(x, y).red() > 150 and small.pixelColor(x, y).green() < 90)
    return red / (small.width() * small.height())


def test_home_from_the_game_zooms_the_frame_into_its_tile(api, fake, monkeypatch):
    from PySide6.QtCore import QMetaObject, QObject
    from PySide6.QtGui import QColor, QImage

    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "SESSION_S", 30.0)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    shot = QImage(640, 360, QImage.Format.Format_RGB32)
    shot.fill(QColor("#d02020"))
    assert shot.save(fake.core.nest_frame())
    _engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    flip = window.findChild(QObject, "homeFlip")
    fake.launch("mirrors-edge", "")
    wait_for(fake.sessionShown, 3000)
    pump(400)
    root.setProperty("tabIndex", 1)
    pump(350)
    swaps = []
    real_focus = fake.core.focus_pid
    monkeypatch.setattr(fake.core, "focus_pid", lambda pid: (swaps.append(flip.property("covering")), real_focus(pid)))
    api.home.toLauncher()
    wait_for(api.home.changed, 3000)
    pump(60)
    assert flip.property("covering") is True and root.property("tabIndex") == 0, "the frame covers the launcher, home first"
    assert red_fraction(window.grabWindow()) > 0.9, "full screen at the swap"
    pump(700)
    assert swaps == [True], "gamescope swapped while the frame covered everything"
    assert flip.property("covering") is False and fake.core.game_shown is False
    home = root.property("activePage")
    assert home.property("currentGame").property("id") == "mirrors-edge", "the cursor lands on the playing game"
    pump(200)
    red = red_fraction(window.grabWindow())
    assert 0.005 < red < 0.2, f"the frame is the tile's art now, nothing more ({red:.3f})"
    QMetaObject.invokeMethod(root, "resumeSession")
    pump(120)
    assert flip.property("growing") is True and api.home.shown == "launcher", "the tile grows first"
    pump(240)
    assert api.home.shown == "game" and fake.core.game_shown is True and red_fraction(window.grabWindow()) > 0.9
    pump(500)
    assert flip.property("covering") is False, "let go once the game has the screen"
    root.setProperty("tabIndex", 1)
    pump(350)
    fake.core.game_shown = False
    pump(400)
    assert api.home.shown == "launcher" and not api.home.flipped
    assert flip.property("covering") is False and root.property("tabIndex") == 1, "a game that leaves by itself gets no zoom of its stale frame"
    stop(api)
    window.close()
    pump(50)


def test_an_output_picked_becomes_current_and_brings_its_level(api, fake):
    home = api.home
    home.loadOutputs()
    wait_for(home.outputsChanged, 3000)
    assert [o["label"] for o in home.outputs if o["current"]] == ["Speakers"]
    headphones = next(o["id"] for o in home.outputs if o["label"] == "Headphones")
    fake.core.level = 30
    home.setOutput(headphones)
    wait_for(home.outputsChanged, 3000)
    assert [o["id"] for o in home.outputs if o["current"]] == [headphones]
    assert home.volumePercent == 30, "the reply is the new sink's level"
    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))
    home.setOutput("gone")
    wait_for(home.outputsChanged, 3000)
    assert errors == ["Invalid"] and [o["id"] for o in home.outputs if o["current"]] == [headphones]
