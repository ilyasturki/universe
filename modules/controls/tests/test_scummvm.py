import _scummvm
import pytest
from _controls import Context, ini_section
from controls_fixtures import EDGE, XBOX


@pytest.fixture
def ini(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "config"))
    return tmp_path / "config" / "scummvm" / "scummvm.ini"


def test_a_first_pad_at_index_zero_needs_nothing(ini):
    assert _scummvm.plan(Context([EDGE])) == {}


def test_a_first_pad_further_down_the_list_is_pointed_at(ini):
    ini.parent.mkdir(parents=True)
    ini.write_text("[scummvm]\nversioninfo=2026.1.0\n\n[monkey2]\ngameid=monkey2\n")
    text = _scummvm.plan(Context([XBOX, EDGE]))[ini]
    assert ini_section(text, "scummvm") == {"versioninfo": "2026.1.0", "joystick_num": "1"}
    assert ini_section(text, "monkey2") == {"gameid": "monkey2"}


def test_a_stale_index_is_reset_and_a_disabled_joystick_left(ini):
    ini.parent.mkdir(parents=True)
    ini.write_text("[scummvm]\njoystick_num=3\n")
    assert ini_section(_scummvm.plan(Context([EDGE]))[ini], "scummvm")["joystick_num"] == "0"
    ini.write_text("[scummvm]\njoystick_num=-1\n")
    assert _scummvm.plan(Context([XBOX])) == {}


def test_the_legacy_scummvmrc_wins(ini, tmp_path):
    rc = tmp_path / ".scummvmrc"
    rc.write_text("[scummvm]\n")
    assert list(_scummvm.plan(Context([XBOX]))) == [rc]
