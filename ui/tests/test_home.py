import os
import threading

import pytest
from looks import Look, invoke, lit_fraction, read, settle
from PySide6.QtCore import QObject, Qt
from PySide6.QtTest import QTest
from uitest import descendant, record, until

from universe_ui import host


def stop(api):
    ended = record(api.universe.sessionEnded)
    api.universe.stop(api.universe.currentSession["session_id"])
    until(lambda: ended)


@pytest.fixture
def held(monkeypatch):
    """`held(obj, name)` holds every call to `obj.name` until the returned event is set; the test's end sets it."""
    gates = []

    def hold(obj, name):
        gate = threading.Event()
        real = getattr(obj, name)

        def gated(*args):
            gate.wait(5)
            return real(*args)

        monkeypatch.setattr(obj, name, gated)
        gates.append(gate)
        return gate

    yield hold
    for gate in gates:
        gate.set()


class Overlay:
    def winId(self):
        return 7

    def show(self):
        pass


def test_a_shot_from_the_pad_or_the_dock_cues_the_same_way(api, fake):
    home = api.home
    taken = []
    home.screenshotTaken.connect(taken.append)
    api.screens.controller._on_event({"event": "screenshot", "path": "/tmp/x.png"})
    assert taken == ["/tmp/x.png"], "the watcher's event reaches the theme through api.home"
    api.screens.controller._on_event({"event": "screenshot", "path": ""})
    assert taken == ["/tmp/x.png", ""], "a failed shot is reported too, silently"
    home.screenshot()
    until(lambda: len(taken) == 3)
    assert taken[2].endswith("screenshot.png"), "the dock's shot goes through the same signal"


def test_a_volume_macro_shows_its_level_on_the_overlay_for_a_moment(api, fake, monkeypatch):
    from universe_ui import home as home_module

    monkeypatch.setattr(home_module, "OSD_MS", 50)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = home_module.Home(fake, api.screens.controller)
    states = []
    monkeypatch.setattr(fake, "overlay", lambda window, input, opacity: states.append((input, opacity)))
    home.attachOverlay(Overlay())
    states.clear()
    api.screens.controller._on_event({"event": "volume", "percent": 40, "muted": False, "output": "Speakers"})
    assert home.osd and (home.volumePercent, home.muted, home.volumeOutput) == (40, False, "Speakers")
    assert states == [(False, home_module.OPAQUE)], "painted over whatever is shown, the pad left where it was"
    until(lambda: not home.osd and states[-1] == (False, 0), "gone again")
    home._open = True
    states.clear()
    api.screens.controller._on_event({"event": "volume", "percent": 45, "muted": False, "output": "Speakers"})
    assert not home.osd and states == [] and home.volumePercent == 45, "the open dock prints the level itself"
    home._open = False
    monkeypatch.delenv("GAMESCOPE_WAYLAND_DISPLAY")
    api.screens.controller._on_event({"event": "volume", "percent": 50, "muted": True, "output": "Speakers"})
    assert not home.osd and states == [], "on the desktop the shell shows it"


def test_a_key_scripts_volume_phase_plays_a_pad_macro(api, fake, monkeypatch):
    from universe_ui import gamepad
    from universe_ui import home as home_module

    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = home_module.Home(fake, api.screens.controller)
    monkeypatch.setattr(fake, "overlay", lambda window, input, opacity: None)
    home.attachOverlay(Overlay())
    before = fake.core.level
    gamepad.KeyScript("Volume:up", 1, None, home=home)._step()
    until(lambda: home.osd, "the overlay shows the level as it does a pad's")
    assert home.volumePercent == fake.core.level > before


def test_home_flips_between_the_game_and_the_launcher(api, fake, monkeypatch):
    from universe_ui import home as home_module

    monkeypatch.setattr(home_module, "COVER_MS", 200)
    home = api.home
    assert home.shown == "launcher" and not home.open
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game", "gamescope's root says the game's window is up")
    assert home.pauseOnHome and not home.paused and fake.core.frozen is False, "on by default, and the game runs while it is shown"
    home.toLauncher()
    until(lambda: home.frame != "")
    assert home.shown == "launcher" and fake.core.game_shown is True, "the frame is taken and offered to the theme before the swap"
    until(lambda: fake.core.game_shown is False, "a theme that never says it has painted the frame still gets the swap")
    until(lambda: home.paused and fake.core.frozen is True, "the launcher over the game: frozen, so the pad drives the menu alone")
    home.changed.connect(home.covered)
    home.toGame()
    until(lambda: home.shown == "game" and fake.core.game_shown is True and fake.core.frozen is False)
    assert not home.paused
    home.setPauseOnHome(False)
    home.toLauncher()
    until(lambda: home.shown == "launcher" and fake.core.game_shown is False)
    assert fake.core.frozen is False, "off for this game: it keeps running behind the launcher"
    home.setPauseOnHome(True)
    until(lambda: fake.core.frozen is True, "turned on while the launcher covers it: frozen now")
    home.toGame()
    until(lambda: fake.core.frozen is False and fake.core.game_shown is True)
    home.openDock()
    until(lambda: home.shown == "launcher" and fake.core.game_shown is False, "no overlay window: HOME goes home instead")
    assert not home.open
    until(lambda: fake.core.frozen is True)
    stop(api)
    assert home.shown == "launcher" and not home.paused


def test_on_the_desktop_the_game_is_shown_once_its_window_maps(api, fake):
    home = api.home
    errors, at_launch = [], []
    fake.error.connect(lambda kind, message: errors.append(kind))
    fake.launched.connect(lambda *args: at_launch.append(home.shown))
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    assert at_launch == ["launcher"], "launched, the window not up yet"
    assert errors == [], "nothing polls gamescope's root outside of it"
    home.toLauncher()
    assert home.shown == "launcher"
    stop(api)


def test_the_dock_pauses_on_home_and_thaws_on_the_release(api, fake):
    home = api.home
    assert home.attachOverlay(object()) is False, "mapped only inside gamescope"
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    home.setPauseOnHome(False)
    home.setPauseOnHome(True)
    assert home.pauseOnHome and fake.game("mirrors-edge")["launch"]["pause_on_home"] is True
    home.guide(True)
    home.openDock()
    until(lambda: fake.core.frozen is True)
    assert home.open and home.paused
    assert api.screens.controller._suspended is True and api.screens.controller._docked is True, (
        "the dock has the pad from the moment it opens; the docked macros still fire"
    )
    home.closeDock()
    home.dockClosed()
    assert not home.open and home.paused and fake.core.frozen is True, "HOME still held: the game stays frozen until the release"
    assert api.screens.controller._suspended is False
    home.guide(False)
    until(lambda: fake.core.frozen is False)
    assert not home.paused
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
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    assert home.attachOverlay(Overlay()) is True
    assert not home.padCovered, "the launcher alone reads the pad"
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game" and home.padCovered, "the game on screen has the pad")
    home.openDock()
    assert home.open and not home.padCovered, "the dock over the game is driven by the pad"
    home.closeDock()
    assert home.padCovered, "closing: the presses are the game's again"
    home.dockClosed()
    stop(api)
    assert not home.padCovered


def test_a_guide_hold_from_the_game_goes_home(api, fake, monkeypatch):
    from universe_ui import home as home_module

    monkeypatch.setattr(home_module, "HOLD_MS", 200)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    presses = []

    def theme_press():
        # What Reprise does with a press: home from the game opens its dock, home from the launcher resumes.
        presses.append(home.shown)
        if home.shown == "launcher":
            home.toGame()

    home.pressed.connect(theme_press)
    home.guide(True)
    home.guide(False)
    assert presses == ["game"] and home.shown == "game", "a tap is the theme's press alone"
    home.guide(True)
    until(lambda: home.shown == "launcher" and fake.core.game_shown is False, "held past hold_ms: the launcher comes up before the release")
    home.guide(False)
    assert home.shown == "launcher" and presses == ["game", "game"]
    home.guide(True)
    until(lambda: not home._hold.isActive(), "held past hold_ms")
    home.guide(False)
    until(lambda: home.shown == "game" and fake.core.game_shown is True)
    assert presses == ["game", "game", "launcher"], "from the launcher a press resumes, and holding it there does not bounce back"
    home.guide(True)
    home.openDock()
    until(lambda: home.shown == "launcher", "no overlay: the press itself went home")
    stop(api)


def test_the_press_takes_the_frame_the_flip_waits_on(api, fake, monkeypatch):
    from universe_ui import fake_core
    from universe_ui import home as home_module

    monkeypatch.setattr(fake_core, "FRAME_S", 0.2)
    monkeypatch.setattr(home_module, "HOLD_MS", 400)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    home.pressed.connect(lambda: home.toGame() if home.shown == "launcher" else None)
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    home.guide(True)
    home.guide(False)
    until(lambda: not home._capturing)
    assert fake.core.frames == 1 and home.shown == "game" and home.frame == "", "a tap asks for the frame; landing late, it flips nothing"
    home.guide(True)
    until(lambda: home.shown == "launcher" and fake.core.game_shown is False and fake.core.frozen is True)
    assert home.frame != "" and fake.core.frames == 2, "the hold flips with the frame its press took, at hold_ms"
    home.guide(False)
    home.toGame()
    until(lambda: fake.core.game_shown is True and fake.core.frozen is False)
    home.guide(True)
    home.toLauncher()
    assert home.shown == "game", "asked while the press's frame is on its way: the flip waits for it"
    until(lambda: home.shown == "launcher" and fake.core.game_shown is False and fake.core.frozen is True)
    assert fake.core.frames == 3, "…and asks for no other"
    home.guide(False)
    home.setPauseOnHome(False)
    home.toGame()
    until(lambda: fake.core.game_shown is True and fake.core.frozen is False)
    home.guide(True)
    home.guide(False)
    until(lambda: not home._capturing)
    monkeypatch.setattr(home_module, "FRESH_S", 0.0)
    frame = home.frame
    home.toLauncher()
    until(lambda: fake.core.frames == 5)
    assert home.shown == "game", "a game that ran on under the dock: the press's frame is stale, a new one is taken"
    until(lambda: home.shown == "launcher" and home.frame not in ("", frame) and fake.core.game_shown is False)
    home.toGame()
    until(lambda: fake.core.game_shown is True)
    with monkeypatch.context() as patched:
        patched.setattr(fake.core, "nest_frame", lambda: None)
        home.toLauncher()
        until(lambda: home.shown == "launcher" and fake.core.game_shown is False, "no frame in time: the swap goes ahead, nothing to zoom")
        assert home.frame == ""
        home.toGame()
        until(lambda: fake.core.game_shown is True)
    assert api.theme.set("switch2")
    home.guide(True)
    home.toLauncher()
    assert home.shown == "launcher" and fake.core.frames == 5, "a look that paints no frame waits for none"
    until(lambda: fake.core.game_shown is False)
    home.guide(False)
    stop(api)


def test_a_theme_that_covers_at_once_gets_the_swap_at_once(api, fake, monkeypatch):
    from universe_ui import home as home_module

    monkeypatch.setattr(home_module, "COVER_MS", 60_000)
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
    until(lambda: home.shown == "game")
    home.toLauncher()
    until(lambda: home.shown == "launcher" and home.paused and fake.core.game_shown is False, "swapped well inside COVER_MS")
    stop(api)


def test_a_hold_that_flips_leaves_nothing_to_thaw_on_the_release(api, fake, monkeypatch):
    from universe_ui import home as home_module

    monkeypatch.setattr(home_module, "HOLD_MS", 200)
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    assert home.attachOverlay(Overlay()) is True
    home.guide(True)
    home.openDock()
    home.closeDock()
    home.dockClosed()
    until(lambda: fake.core.frozen is True, "closed while HOME is held: frozen until the release")
    assert home.paused
    until(lambda: home.shown == "launcher", "the hold went home meanwhile")
    assert home.paused
    home.guide(False)
    assert home.paused and fake.core.frozen is True, "the release thaws nothing under the launcher"
    stop(api)


def test_quitting_from_the_game_brings_the_launcher_up_first(api, fake, monkeypatch):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    home.changed.connect(home.covered)
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    titles, ended, stops = [], [], []
    home.stopping.connect(titles.append)
    api.universe.sessionEnded.connect(lambda *a: ended.append(a))
    real_stop = fake.core.stop
    monkeypatch.setattr(fake.core, "stop", lambda sid: (stops.append((fake.core.game_shown, fake.core.frozen)), real_stop(sid)))
    home.stop()
    home.stop()
    assert titles == ["Mirror's Edge"], "a second Quit while one is under way is nothing"
    until(lambda: ended)
    assert stops == [(False, False)], "the launcher is up, and the game never frozen, when it is asked to quit: the SIGTERM has to land"
    assert len(ended) == 1 and home.shown == "launcher" and not home.paused


def test_the_hud_and_the_limit_reach_the_game_without_a_key(api, fake):
    from universe_ui.screens.controller import FakeWatcher

    home = api.home
    home.attachOverlay(object())
    watcher = FakeWatcher("dualsense-edge")
    api.screens.controller.restart_ms = 0
    api.screens.controller.start(watcher)
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    assert home.runtime()["mangohud"] is False, "off by default, hidden at launch"
    home.setRuntime("mangohud", "true")
    until(lambda: home.runtime()["mangohud"] is True)
    home.setRuntime("mangohud", "false")
    until(lambda: home.runtime()["mangohud"] is False)
    assert not any(c.get("action") == "keys" for c in watcher.commands), "no key typed for the HUD"

    home.setPauseOnHome(True)
    home.openDock()
    until(lambda: fake.core.frozen is True)
    assert home.paused
    home.setRuntime("fps_limit", "60")
    home.setRuntime("fps_limit", "30")
    until(lambda: fake.core.fps_limit_writes == 2, "each change rewrites the layer's conf, frozen or not")
    assert home.runtime()["fps_limit"] == "30"
    home.closeDock()
    home.dockClosed()
    until(lambda: fake.core.frozen is False)
    assert not home.paused
    assert not any(c.get("action") == "keys" for c in watcher.commands), "MangoHud rereads its conf by itself: no key typed, before or after the thaw"
    launch = fake.game("mirrors-edge").get("launch") or {}
    assert launch.get("mangohud") is None and launch.get("fps_limit") in (None, ""), "the running game's alone: its settings stand"
    stop(api)
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    assert home.runtime()["mangohud"] is False and home.runtime()["fps_limit"] != "30", "the next launch starts from the settings"
    stop(api)


def test_the_dock_opens_over_the_poster_before_the_session_is_made_and_grows_once_the_window_is_up(api, fake, monkeypatch, held):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    made = held(fake.core, "launch")
    mapped = held(fake.core, "wait_session_window")
    home = api.home
    home.attachOverlay(Overlay())
    assert not home.loading
    launched = record(fake.launched)
    fake.launch("mirrors-edge", "")
    assert fake.currentSession is None and home.pending == {"id": "mirrors-edge", "title": "Mirror's Edge"} and home.loading
    home.openDock()
    assert home.open and not home.paused and fake.core.frozen is False, "HOME answers while the pads are still being taken"
    made.set()
    until(lambda: launched)
    assert home.pending is None and home.open and home.loading and home.shown == "launcher", "the session made, no window yet: the same dock stays"
    home.setPauseOnHome(True)
    assert fake.core.frozen is False, "the dock over the poster freezes nothing: the game is still starting"
    mapped.set()
    until(
        lambda: not home.loading and home.shown == "game" and fake.core.frozen is True,
        "the window is up: the game freezes under the dock, as pause on HOME asks",
    )
    assert home.open and home.paused, "the dock stays, grown to the full one"
    home.closeDock()
    home.dockClosed()
    until(lambda: fake.core.frozen is False)
    assert not home.paused
    stop(api)
    assert not home.loading


@pytest.mark.parametrize("made", [True, False], ids=["session", "pending"])
def test_quit_from_the_dock_over_a_loading_game_stops_it_through_the_launcher(api, fake, monkeypatch, held, made):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    making = held(fake.core, "launch")
    held(fake.core, "wait_session_window")
    home = api.home
    home.attachOverlay(Overlay())
    quitting = record(home.stopping)
    launched, ended = record(fake.launched), record(fake.sessionEnded)
    fake.launch("mirrors-edge", "")
    if made:
        making.set()
        until(lambda: launched)
    home.openDock()
    home.stop()
    assert quitting == [("Mirror's Edge",)] and not home.open and home.flipped and fake.core.frozen is False, "the launcher comes up, nothing frozen"
    making.set()
    until(lambda: ended, "stopped, as soon as the session existed")
    assert home.pending is None and not home.loading and not home.flipped and home.shown == "launcher"
    assert fake.core.frames == 0, "no frame was asked for"


def test_a_launch_that_fails_before_its_session_takes_the_dock_down(api, fake, monkeypatch, held):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    made = held(fake.core, "launch")
    home = api.home
    home.attachOverlay(Overlay())
    failed = record(fake.launchFailed)
    fake.launch("no-such-game", "")
    home.openDock()
    assert home.open
    made.set()
    until(lambda: failed)
    assert home.pending is None and not home.open and not home.loading


def test_sharpness_reaches_gamescope_with_the_running_filter(api, fake, monkeypatch):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    home = api.home
    fake.launch("mirrors-edge", "")
    until(lambda: home.shown == "game")
    assert home.runtime()["gamescope_sharpness"] is None and home.launchChoices("gamescope_sharpness") == ["0", "2", "5", "10", "15", "20"]
    home.setRuntime("gamescope_filter", "fsr")
    assert fake.core.filter == ("fsr", None), "no sharpness set: the card goes, gamescope's default holds"
    home.setRuntime("gamescope_sharpness", "5")
    assert fake.core.filter == ("fsr", 5) and home.runtime()["gamescope_sharpness"] == 5, "applied with the filter the game runs with"
    assert (fake.game("mirrors-edge").get("launch") or {}).get("gamescope_sharpness") in (None, ""), "the game's settings stand"
    home.setRuntime("gamescope_sharpness", "")
    assert fake.core.filter == ("fsr", None)
    stop(api)
    assert fake.core.filter == ("", None), "the session's end puts the settings' filter back"


def covered_fraction(image):
    small = image.scaled(96, 54)
    covered = sum(1 for y in range(small.height()) for x in range(small.width()) if small.pixelColor(x, y).alpha() > 10)
    return covered / (small.width() * small.height())


def key(window, k, times=1):
    for _ in range(times):
        QTest.keyClick(window, k)


@pytest.fixture
def reprise(api):
    """Reprise's window, and the overlay its dock draws on over the game: (look, overlay)."""
    look = Look(api, "reprise")
    overlay = host.create_overlay(look.engine, look.window.size())
    api.home.attachOverlay(overlay)
    overlay.show()
    settle(overlay)
    yield look, overlay
    overlay.close()
    look.close()


def open_dock(api, overlay):
    api.home.openDock()
    overlay.requestActivate()
    dock = overlay.findChild(QObject, "dock")
    until(lambda: dock.property("activeFocus"))
    return dock


def band_up(overlay):
    band = overlay.findChild(QObject, "dockBand")
    until(lambda: band.property("opacity") == 1.0, "the band faded in")


def ids(items):
    return [i["id"] for i in items]


def current_output(api):
    return next(o["id"] for o in api.home.outputs if o["current"])


def test_the_dock_renders_over_a_running_game(api, fake, reprise, tmp_path):
    look, overlay = reprise
    root = look.root
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    dock = open_dock(api, overlay)
    band_up(overlay)
    settle(overlay)
    image = overlay.grabWindow()
    assert 0.3 < covered_fraction(image) < 0.6 and lit_fraction(image, "#000000") > 0.01, (
        "the band covers the lower part of the frame, the card and the buttons drawn on it"
    )
    assert image.pixelColor(4, 4).alpha() == 0, "the top of the frame stays clear"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        image.save(str(tmp_path / "dock.png"))
    buttons = ids(read(dock, "buttons"))
    assert {"resume", "home", "quit", "game"} <= set(buttons), "Quit a button of its own"
    key(overlay, Qt.Key.Key_Right, buttons.index("game"))
    assert dock.property("index") == buttons.index("game")
    key(overlay, Qt.Key.Key_Return)
    assert dock.property("opened") is True, "Game opens its card"
    game = ids(read(dock, "current")["children"])
    assert set(game) == {"details", "journal", "recordings", "sessions"}
    key(overlay, Qt.Key.Key_Escape)
    dock.setProperty("index", buttons.index("perf"))
    key(overlay, Qt.Key.Key_Return)
    assert api.home.pauseOnHome is True
    dock.setProperty("sub", ids(read(dock, "current")["children"]).index("pause"))
    key(overlay, Qt.Key.Key_Return)
    assert api.home.pauseOnHome is False, "on by default: A on Pause on HOME turns it off"
    key(overlay, Qt.Key.Key_Escape)
    assert dock.property("opened") is False
    key(overlay, Qt.Key.Key_Escape)
    assert api.home.open is False, "B closes the dock"

    open_dock(api, overlay)
    dock.setProperty("index", buttons.index("home"))
    key(overlay, Qt.Key.Key_Return)
    until(lambda: api.home.shown == "launcher" and api.home.open is False and fake.core.game_shown is False, "Home from the dock: the launcher")
    assert root.property("detailOpen") is False, "Home from the dock: nothing opened"

    def from_the_game_card(landing):
        api.home.toGame()
        until(lambda: fake.core.game_shown is True)
        root.goToTab(root.property("settingsTab"))
        open_dock(api, overlay)
        dock.setProperty("index", buttons.index("game"))
        key(overlay, Qt.Key.Key_Return)
        dock.setProperty("sub", game.index(landing))
        key(overlay, Qt.Key.Key_Return)
        until(lambda: api.home.shown == "launcher" and api.home.open is False and fake.core.game_shown is False, f"{landing} in the Game card goes home")

    from_the_game_card("details")
    look.page("homePage")
    until(lambda: root.property("detailOpen") is True, "…lands on Home and opens the playing game's details there")
    assert api.home.takeLanding() == "", "taken once"
    from_the_game_card("journal")
    look.page("journalPage")
    api.home.toGame()
    until(lambda: fake.core.game_shown is True)
    stop(api)


def test_the_docks_achievements_open_in_its_tray_while_the_source_tracks_them(api, fake, reprise, tmp_path):
    _look, overlay = reprise
    fake.launch("batman-arkham-origins", "")
    until(lambda: api.home.shown == "game")
    dock = open_dock(api, overlay)
    buttons = ids(read(dock, "buttons"))
    assert "achievements" in buttons, "a button of its own"
    dock.setProperty("index", buttons.index("achievements"))
    key(overlay, Qt.Key.Key_Return)
    tray, trophies = overlay.findChild(QObject, "dockShots"), overlay.findChild(QObject, "dockAchievements")
    assert tray.property("open") is True and tray.property("showsAchievements") is True
    assert api.home.shown != "launcher", "over the game, not the launcher"
    until(lambda: len(trophies.property("rows")) == 6)
    assert api.screens.dockAchievements.gameId == "batman-arkham-origins"
    key(overlay, Qt.Key.Key_Down)
    assert trophies.property("index") == 1
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(str(tmp_path / "dock-achievements.png"))
    key(overlay, Qt.Key.Key_Q)
    assert tray.property("showsAchievements") is False, "LB back to the screenshots"
    key(overlay, Qt.Key.Key_E)
    key(overlay, Qt.Key.Key_Up, 2)
    assert tray.property("open") is False, "Up past the first closes the tray"

    fake.set("batman-arkham-origins", "sources.gog.achievements", "false")
    api.home.closeDock()
    open_dock(api, overlay)
    until(lambda: "achievements" not in ids(read(dock, "buttons")), "the source's switch off: no button")
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_E)
    assert tray.property("open") is True and tray.property("showsAchievements") is False, "…and no tab"
    stop(api)


def test_a_burst_of_unlocks_waits_its_turn_behind_three_cards(api, reprise, tmp_path, monkeypatch):
    from universe_ui import home as home_module

    # Over 700 ms: a card's fades and margin take that much of its life.
    monkeypatch.setattr(home_module, "BANNER_MS", 800)
    _look, overlay = reprise
    for n in range(7):
        api.home._unlocked({"name": f"A{n}", "description": "", "icon": "", "rarityText": ""})
    unlocks, cards = overlay.findChild(QObject, "unlocks"), overlay.findChild(QObject, "unlockCards")
    until(lambda: cards.property("count") == 3)
    assert api.home.bannersWaiting == 4
    assert unlocks.property("x") + unlocks.property("width") > overlay.width() * 0.9, "top right, clear of the game's middle"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(str(tmp_path / "unlocks.png"))
    until(lambda: api.home.bannersWaiting == 1 and cards.property("count") == 3, "the next three came in as the first left")


def test_the_docks_output_row_switches_once_the_cursor_rests(api, fake, reprise, tmp_path, monkeypatch):
    switched = []
    real_set = fake.core.set_output
    monkeypatch.setattr(fake.core, "set_output", lambda ident: (switched.append(ident), real_set(ident))[1])
    _look, overlay = reprise
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    outputs = record(api.home.outputsChanged)
    dock = open_dock(api, overlay)
    until(lambda: outputs)
    dock.setProperty("index", ids(read(dock, "buttons")).index("sound"))
    key(overlay, Qt.Key.Key_Return)
    rows = ids(read(dock, "current")["children"])
    dock.setProperty("sub", rows.index("vol"))
    key(overlay, Qt.Key.Key_Return)
    until(lambda: api.home.muted is True, "Mute is A on Volume")
    key(overlay, Qt.Key.Key_Return)
    until(lambda: api.home.muted is False)
    dock.setProperty("sub", rows.index("output"))
    assert read(dock, "target")["id"] == "output" and read(dock, "vals")["output"].endswith("speaker")
    key(overlay, Qt.Key.Key_Right, 2)
    assert read(dock, "vals")["output"].endswith("hdmi-output-0"), "the row shows the pick at once"
    assert current_output(api).endswith("speaker"), "nothing switched while stepping"
    until(lambda: current_output(api).endswith("hdmi-output-0"))
    assert len(switched) == 1 and switched[0].endswith("hdmi-output-0"), "one switch, to where the cursor rested"
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        overlay.grabWindow().save(str(tmp_path / "dock-output.png"))
    stop(api)


# Each look's dock, and the property and list its cursor takes the Sound group from.
SOUND = {"reprise": ("dock", "index", "buttons"), "ps5": ("controlCenter", "icon", "icons")}


@pytest.mark.parametrize("look", list(SOUND), indirect=True)
def test_the_volume_row_keeps_the_level_dimmed_under_the_mute_and_up_unmutes(api, fake, look):
    overlay = host.create_overlay(look.engine, look.window.size())
    api.home.attachOverlay(overlay)
    overlay.show()
    settle(overlay)
    name, cursor, buttons = SOUND[look.name]
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    fake.core.level, fake.core.muted = 60, True
    api.home.openDock()
    dock = until(lambda: overlay.findChild(QObject, name))
    until(lambda: (api.home.volumePercent, api.home.muted) == (60, True), "the dock reads the level as it opens")
    dock.setProperty(cursor, ids(read(dock, buttons)).index("sound"))
    if look.name == "ps5":
        dock.setProperty("zone", "panel")
    level = until(lambda: descendant(overlay.contentItem(), "volumeLevel"))
    fill, glyph = descendant(level, "volumeFill"), descendant(overlay.contentItem(), "volumeGlyph")
    until(lambda: glyph.property("kind") == "mute" and level.property("opacity") < 1)
    assert fill.property("width") > 0, "the level stays drawn under the mute"
    api.home.volume("up", 0)
    until(lambda: (api.home.volumePercent, api.home.muted) == (62, False), "up unmutes as it steps")
    until(lambda: glyph.property("kind") != "mute" and level.property("opacity") == 1)
    api.home.stop()
    until(lambda: api.universe.currentSession is None)
    overlay.close()


def test_home_over_the_poster_raises_home_and_quit_and_home_drops_the_poster(api, fake, reprise, tmp_path, monkeypatch, held):
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    mapped = held(fake.core, "wait_session_window")
    look, overlay = reprise
    root = look.root
    poster = look.find("launchOverlay")
    dock = overlay.findChild(QObject, "dock")
    started = record(fake.sessionStarted)
    look.launch("control")
    api.home.pressed.emit()
    assert api.home.open is False, "HOME before the session exists is swallowed with the other keys"
    until(lambda: started)
    until(lambda: poster.property("waiting") is True)
    assert api.home.loading is True
    api.home.pressed.emit()
    overlay.requestActivate()
    until(lambda: dock.property("activeFocus"))
    assert api.home.open is True and dock.property("loading") is True
    assert ids(read(dock, "buttons")) == ["home", "quit"], "over the poster: Home and Quit alone"
    assert api.home.paused is False and fake.core.frozen is False
    if os.environ.get("UNIVERSE_TEST_SHOTS"):
        band_up(overlay)
        settle(overlay)
        overlay.grabWindow().save(str(tmp_path / "dock-loading.png"))
    key(overlay, Qt.Key.Key_Down)
    assert overlay.findChild(QObject, "dockShots").property("open") is False, "no shots while loading"
    api.home.pressed.emit()
    until(lambda: api.home.open is False, "a second press closes it, the poster still holds")
    assert poster.property("waiting") is True
    api.home.pressed.emit()
    overlay.requestActivate()
    until(lambda: api.home.open and dock.property("activeFocus"))
    key(overlay, Qt.Key.Key_Return)
    until(lambda: api.home.open is False and poster.property("waiting") is False, "Home: the poster goes")
    assert api.home.flipped and api.home.frame == "" and fake.core.frames == 0, "nothing painted yet: no frame asked for"
    assert fake.core.frozen is False and root.property("playingId") == "control"
    mapped.set()
    until(lambda: not api.home.loading and not api.home.flipped and api.home.shown == "game", "the game mapped over the launcher on its own")
    dock = open_dock(api, overlay)
    buttons = read(dock, "buttons")
    assert {"resume", "home", "quit", "game"} <= set(ids(buttons)), "the full dock once the game is up"
    assert buttons[dock.property("index")]["id"] == "resume"
    key(overlay, Qt.Key.Key_Escape)
    stop(api)


def test_the_dock_lists_the_sessions_shots_and_trashes_one(api, fake, reprise, tmp_path):
    _look, overlay = reprise
    fake.launch("the-technomancer", "")
    until(lambda: api.home.shown == "game")
    earlier = len(fake.screenshots("the-technomancer"))
    assert earlier > 0, "the fixture seeds shots on past sessions"
    taken = record(api.home.screenshotTaken)
    api.home.screenshot()
    until(lambda: taken)
    dock = open_dock(api, overlay)
    panel = overlay.findChild(QObject, "dockShots")
    assert panel.property("open") is False
    key(overlay, Qt.Key.Key_Down)
    assert panel.property("open") is True, "▼ from the row raises the screenshots over the game"
    until(lambda: panel.property("count") == earlier + 1)
    assert api.screens.shots.gameId == "the-technomancer"
    assert panel.property("mine") == 1, "the shot just taken sits under THIS SESSION, the seeded ones under EARLIER"
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
    key(overlay, Qt.Key.Key_Down)
    key(overlay, Qt.Key.Key_Return)
    until(lambda: panel.property("count") == earlier)
    assert panel.property("mine") == 0, "trashed: the list follows the directory"
    assert len(fake.screenshots("the-technomancer")) == earlier
    key(overlay, Qt.Key.Key_Up)
    assert panel.property("open") is False and api.home.open is True, "▲ past the top row lowers the panel onto the dock"
    key(overlay, Qt.Key.Key_Escape)
    assert api.home.open is False
    stop(api)


def red_fraction(image):
    small = image.scaled(96, 54)
    red = sum(1 for y in range(small.height()) for x in range(small.width()) if small.pixelColor(x, y).red() > 150 and small.pixelColor(x, y).green() < 90)
    return red / (small.width() * small.height())


@pytest.mark.parametrize("look", ["reprise"], indirect=True)
def test_home_from_the_game_zooms_the_frame_into_its_tile(api, fake, monkeypatch, look):
    from PySide6.QtGui import QColor, QImage

    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    shot = QImage(640, 360, QImage.Format.Format_RGB32)
    shot.fill(QColor("#d02020"))
    assert shot.save(fake.core.nest_frame())
    root, window = look.root, look.window
    elsewhere = root.property("settingsTab")
    flip = look.find("homeFlip")
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    root.goToTab(elsewhere)
    swaps = []
    real_focus = fake.core.focus_pid
    monkeypatch.setattr(fake.core, "focus_pid", lambda pid: (swaps.append(flip.property("covering")), real_focus(pid)))
    api.home.toLauncher()
    until(lambda: red_fraction(window.grabWindow()) > 0.9, "full screen at the swap")
    assert flip.property("covering") is True and read(root, "activePage").objectName() == "homePage", "the frame covers the launcher, home first"
    until(lambda: not flip.property("covering") and not flip.property("running"))
    assert swaps == [True], "gamescope swapped while the frame covered everything"
    assert fake.core.game_shown is False
    assert look.page("homePage").property("currentGame").property("id") == "mirrors-edge", "the cursor lands on the playing game"
    until(lambda: 0.005 < red_fraction(window.grabWindow()) < 0.2, "the frame is the tile's art now")
    invoke(root, "resumeSession")
    assert flip.property("growing") is True and api.home.shown == "launcher", "the tile grows first"
    until(lambda: api.home.shown == "game" and fake.core.game_shown is True)
    assert flip.property("covering") is True, "grown to the full frame over the swap back"
    until(lambda: flip.property("covering") is False, "let go once the game has the screen")
    root.goToTab(elsewhere)
    fake.core.game_shown = False
    until(lambda: api.home.shown == "launcher")
    assert not api.home.flipped
    assert flip.property("covering") is False and root.property("tabIndex") == elsewhere, "a game that leaves by itself gets no zoom of its stale frame"
    stop(api)


def test_an_output_picked_becomes_current_and_brings_its_level(api, fake):
    home = api.home
    changed = record(home.outputsChanged)
    home.loadOutputs()
    until(lambda: changed)
    assert current_output(api).endswith("speaker")
    headphones = next(o["id"] for o in home.outputs if o["id"].endswith("headphones"))
    fake.core.level = 30
    home.setOutput(headphones)
    until(lambda: len(changed) == 2)
    assert current_output(api) == headphones
    assert home.volumePercent == 30, "the reply is the new sink's level"
    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))
    home.setOutput("gone")
    until(lambda: errors)
    assert errors == ["Invalid"] and current_output(api) == headphones
