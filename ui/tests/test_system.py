import pytest

from conftest import pump, wait_for
from universe_ui.api import Api
from universe_ui.screens.power import FAKE


@pytest.fixture
def deck_api(monkeypatch, fake, tmp_path):
    monkeypatch.setenv("UNIVERSE_DECK", "oled")
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    wait_for(api.system.controlsChanged)
    yield api
    api.shutdown()


def test_a_deck_lists_its_controls_after_putting_back_the_kept_ones(deck_api, fake):
    assert [c["id"] for c in deck_api.system.controls] == ["brightness", "refresh", "tdp", "gpu", "fan"]
    assert fake._core.system_applied == 1, "[system] goes back before the rows are read"
    assert deck_api.system.deck == "oled" and deck_api.system.steam is False


def test_a_set_shows_at_once_and_reaches_the_core(deck_api, fake):
    deck_api.system.set("tdp", "9")
    assert deck_api.system.control("tdp")["value"] == "9", "the row moves before the helper answers"
    pump(200)
    assert next(c for c in fake._core.system if c["id"] == "tdp")["value"] == "9"


def test_a_refused_control_says_why_and_reads_the_machine_back(deck_api, fake):
    fake._core.system_error = "steamos-priv-write refused /sys/class/hwmon/hwmon5/power1_cap"
    deck_api.system.set("tdp", "4")
    failed = wait_for(deck_api.system.controlFailed)
    assert failed == ("tdp", "steamos-priv-write refused /sys/class/hwmon/hwmon5/power1_cap")
    wait_for(deck_api.system.controlsChanged)
    assert deck_api.system.control("tdp")["value"] == "15"


def test_a_desktop_has_no_system_controls(api):
    pump(200)
    assert api.system.controls == [] and api.system.deck == ""


def test_steams_game_mode_keeps_power_and_the_machines_controls(monkeypatch, fake, tmp_path):
    monkeypatch.setenv("UNIVERSE_DECK", "lcd")
    monkeypatch.setenv("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")
    monkeypatch.setenv("UNIVERSE_FAKE_STEAM", "1")
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    pump(300)
    assert api.system.steam is True
    assert list(api.system.actions) == [] and api.system.controls == [], "Steam's Quick Access menu has them"
    keys = [spec["key"] for spec in fake.launchKeys("global", None)]
    assert "mangohud" not in keys and "fps_limit" not in keys and "pause_on_home" not in keys
    assert "hdr" in keys or "gamescope" in keys
    api.shutdown()


def test_the_decks_own_glyphs_until_a_pad_says_otherwise(monkeypatch, fake, tmp_path):
    monkeypatch.setenv("UNIVERSE_DECK", "lcd")
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    assert api.screens.controller.family == "steam-deck", "Steam or SDL may keep the built-in controls from the watcher"
    api.shutdown()
