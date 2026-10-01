import pytest
from PySide6.QtCore import Qt

from conftest import record, until
from universe_ui import focus as focus_module
from universe_ui.focus import Focus


@pytest.fixture
def unfocused(fake, tmp_path, monkeypatch):
    """An Api whose focus comes from the fake desktop, as inside the launcher's nested gamescope."""
    from universe_ui.api import Api
    from universe_ui.screens.network import FAKE as FAKE_NET
    from universe_ui.screens.power import FAKE

    monkeypatch.setattr(focus_module, "POLL_MS", 20)
    monkeypatch.setattr(focus_module, "mode_of", lambda client: "host")
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE, net_root=FAKE_NET)
    yield api
    api.shutdown()


@pytest.mark.parametrize(
    ("desktop", "active", "elsewhere"),
    [("launcher", True, False), ("session", False, False), ("other", False, True), ("unknown", True, False)],
)
def test_the_desktop_says_who_has_the_focus(fake, monkeypatch, desktop, active, elsewhere):
    monkeypatch.setattr(focus_module, "POLL_MS", 20)
    fake.core.focus = "launcher" if desktop == "other" else "other"
    focus = Focus(fake, mode="host")
    until(lambda: focus.elsewhere is (desktop != "other"), "a first answer")
    fake.core.focus = desktop
    until(lambda: (focus.active, focus.elsewhere) == (active, elsewhere), f"{desktop}: never {(active, elsewhere)}")
    focus.shutdown()


def test_on_the_desktop_qt_says_and_the_desktop_names_the_other_window(fake, monkeypatch):
    monkeypatch.setattr(focus_module, "POLL_MS", 20)
    fake.core.focus = "other"
    focus = Focus(fake, mode="qt")
    focus._on_state(Qt.ApplicationState.ApplicationActive)
    assert focus.active and not focus.elsewhere, "Qt's own word, with no desktop asked"
    focus._on_state(Qt.ApplicationState.ApplicationInactive)
    until(lambda: focus.elsewhere)
    fake.core.focus = "session"
    until(lambda: not focus.elsewhere)
    assert not focus.active, "the game's window has it: HOME stays HOME"
    focus._on_state(Qt.ApplicationState.ApplicationActive)
    assert focus.active
    focus.shutdown()


def test_nothing_else_has_the_focus_on_its_own_screen_or_under_steam(fake, monkeypatch):
    monkeypatch.setenv("UNIVERSE_OWN_GAMESCOPE", "drm")
    assert focus_module.mode_of(fake) == "always"
    monkeypatch.delenv("UNIVERSE_OWN_GAMESCOPE")
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    assert focus_module.mode_of(fake) == "host"
    monkeypatch.setenv("UNIVERSE_FAKE_STEAM", "1")
    assert focus_module.mode_of(fake) == "always"


def test_b_held_as_the_focus_leaves_opens_no_power_menu(unfocused, fake):
    held = record(unfocused.keys.cancelHeld)
    unfocused.keys._hold.start()
    fake.core.focus = "other"
    until(lambda: not unfocused.focus.active)
    assert not unfocused.keys._hold.isActive() and held == []


def test_the_controller_page_stops_reading_the_pad_when_the_focus_leaves(unfocused, fake):
    from universe_ui.screens.controller import FakeWatcher

    screen = unfocused.screens.controller
    watcher = FakeWatcher("dualsense-edge")
    screen.start(watcher)
    assert screen.setTesting(True) and screen.testing
    fake.core.focus = "other"
    until(lambda: not unfocused.focus.active)
    assert not screen.testing and {"cmd": "axes", "on": False} in watcher.commands
    fake.core.focus = "launcher"
    until(lambda: unfocused.focus.active)
    assert screen.learn("south") and screen.learning == "south"
    fake.core.focus = "other"
    until(lambda: not unfocused.focus.active)
    assert screen.learning == "" and watcher.commands[-1] == {"cmd": "cancel"}
    fake.core.focus = "launcher"
    until(lambda: unfocused.focus.active)
    assert screen.startWalk() and screen.walking
    fake.core.focus = "other"
    until(lambda: not unfocused.focus.active)
    assert not screen.walking and screen.learning == ""


def guide(api, pressed):
    api.screens.controller._on_event({"event": "button", "id": "event30", "slot": "guide", "pressed": pressed})


@pytest.mark.parametrize(("desktop", "summoned", "home"), [("other", 1, False), ("session", 0, True), ("launcher", 0, True), ("unknown", 0, True)])
def test_home_from_another_app_brings_the_launcher_up_and_nothing_more(unfocused, fake, desktop, summoned, home):
    fake.core.focus = desktop
    until(lambda: unfocused.focus.elsewhere is (desktop == "other"))
    pressed = record(unfocused.home.pressed)
    guide(unfocused, True)
    guide(unfocused, False)
    until(lambda: fake.core.summons == summoned)
    assert bool(pressed) is home, "HOME acts where the launcher or its game has the focus, or nobody can tell"
    if summoned:
        until(lambda: unfocused.focus.active, "asked again at once")


def test_home_from_another_app_does_nothing_with_home_summons_off(unfocused, fake):
    fake.setConfig("controller.home_summons", "false")
    unfocused.screens.controller.load()
    fake.core.focus = "other"
    until(lambda: unfocused.focus.elsewhere)
    pressed = record(unfocused.home.pressed)
    guide(unfocused, True)
    assert not unfocused.home._hold.isActive(), "no hold is timed"
    guide(unfocused, False)
    assert fake.core.summons == 0 and pressed == []
