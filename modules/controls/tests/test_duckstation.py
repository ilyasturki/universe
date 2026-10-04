import _duckstation
import pytest
from _controls import Context, ini_section
from controls_fixtures import EDGE, SWITCH_PRO, XBOX

DUCK_INI = """[Main]
SettingsVersion = 3

[ControllerPorts]
MultitapMode = Disabled

[Pad1]
Type = AnalogController
Cross = SDL-0/A
Up = SDL-5D3F-1234/DPadUp

[InputSources]
SDLJoystickXboxHIDAPI = false
"""


@pytest.fixture
def duck(tmp_path, monkeypatch):
    monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
    ini = tmp_path / ".local/share/duckstation/settings.ini"
    ini.parent.mkdir(parents=True)
    ini.write_text(DUCK_INI)
    return ini


def test_duckstation_binds_sdls_positional_names_over_the_old_ones(duck):
    text = _duckstation.plan(Context([SWITCH_PRO, XBOX]))[duck]
    pad1 = ini_section(text, "Pad1")
    assert (pad1["Cross"], pad1["Circle"], pad1["Triangle"], pad1["Up"]) == ("SDL-2/A", "SDL-2/B", "SDL-2/Y", "SDL-2/DPadUp")
    assert "SDL-5D3F" not in text
    assert (ini_section(text, "Pad2")["Type"], ini_section(text, "Pad2")["Cross"]) == ("AnalogController", "SDL-1/A")


def test_duckstation_looks_in_an_absolute_xdg_config_home_first(duck, tmp_path, monkeypatch):
    xdg = tmp_path / "xdg"
    (xdg / "duckstation").mkdir(parents=True)
    (xdg / "duckstation" / "settings.ini").write_text(DUCK_INI)
    monkeypatch.setenv("XDG_CONFIG_HOME", str(xdg))
    assert _duckstation.config_path() == xdg / "duckstation" / "settings.ini"
    monkeypatch.setenv("XDG_CONFIG_HOME", "relative")
    assert _duckstation.config_path() == duck


def test_duckstation_multitap_mode_opens_more_ports(duck):
    duck.write_text(DUCK_INI.replace("MultitapMode = Disabled", "MultitapMode = Port1Only"))
    text = _duckstation.plan(Context([EDGE, XBOX, SWITCH_PRO]))[duck]
    assert ini_section(text, "Pad3")["Cross"] == "SDL-2/A"


def test_duckstation_hints_follow_its_input_sources(duck):
    hints = _duckstation.hints()
    assert hints["SDL_JOYSTICK_HIDAPI_XBOX"] == "0" and hints["SDL_JOYSTICK_LINUX_DIGITAL_HATS"] == "0"
    assert hints["SDL_JOYSTICK_ENHANCED_REPORTS"] == "0" and hints["SDL_JOYSTICK_HIDAPI_PS3"] == "1"
