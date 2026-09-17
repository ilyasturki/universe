import os

from conftest import pump, wait_for
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from test_render import lit_fraction, render
from universe_ui import host


def stop(api):
    api.universe.stop(api.universe.currentSession["session_id"])
    wait_for(api.universe.sessionEnded, 5000)
    pump(50)


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
    pump(400)
    assert home.shown == "launcher" and fake.core.game_shown is False
    assert home.paused and fake.core.frozen is True, "the launcher over the game: frozen, so the pad drives the menu alone"
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
    assert api.screens.controller._suspended is False, "the macros run while HOME is held: a hold on it still counts"
    home.closeDock()
    home.dockClosed()
    pump(50)
    assert not home.open and home.paused and fake.core.frozen is True, "HOME still held: the game stays frozen until the release"
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
    home.openDock()
    assert api.screens.controller._suspended is True, "opened without HOME: nothing to wait for"
    stop(api)
    assert not home.open and home.shown == "launcher"


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
    assert lit_fraction(image, "#000000") > 0.01, "the card, the pill and the hints are drawn on it"
    key(overlay, Qt.Key.Key_Right)
    key(overlay, Qt.Key.Key_Right)
    assert dock.property("index") == 2
    key(overlay, Qt.Key.Key_Return)
    assert dock.property("opened") is True, "Game opens its card"
    assert api.home.pauseOnHome is True
    key(overlay, Qt.Key.Key_Return)
    assert api.home.pauseOnHome is False, "the first row is Pause on HOME, on by default: A turns it off"
    key(overlay, Qt.Key.Key_Escape)
    assert dock.property("opened") is False
    key(overlay, Qt.Key.Key_Escape)
    pump(400)
    assert api.home.open is False, "B closes the dock"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        image.save(str(tmp_path / "dock.png"))
    stop(api)
    window.close()
    overlay.close()
    pump(50)
